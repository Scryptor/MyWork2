//! Печать через CUPS (macOS, Linux): принтеры — `lpstat`, печать — `lp` с PDF во временном файле.
//!
//! Имя принтера и путь к файлу — отдельные аргументы (`Command::args`, без оболочки). Вывод `lpstat` разбирается
//! в локали C; ошибку `lp` человек читает как есть, на языке своей системы.

use super::{MAX_DOCUMENT_BYTES, PrinterInfo, check_copies, decode_base64, known_printer};
use std::ffi::{OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::ops::RangeInclusive;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Программа печати CUPS.
const LP: &str = "lp";
/// Программа состояния CUPS.
const LPSTAT: &str = "lpstat";
/// Сторона наклейки, мм.
const MEDIA_MM: RangeInclusive<f32> = 15.0..=200.0;
/// Сколько имён временного файла пробуется, если имя занято.
const TEMP_TRIES: usize = 16;

/// Принтеры CUPS; без CUPS (нет `lpstat`) — пустой список.
pub(super) fn printers() -> Result<Vec<PrinterInfo>, String> {
    printers_from(OsStr::new(LPSTAT))
}

/// Печатает PDF на `printer`: сначала проверки (копии, наклейка, документ, принтер), затем `lp`.
pub(super) fn print_pdf(
    printer: &str,
    pdf_base64: &str,
    copies: u32,
    media_mm: Option<(f32, f32)>,
) -> Result<(), String> {
    check_copies(copies)?;
    let media = check_media(media_mm)?;
    let pdf = decode_base64(pdf_base64, MAX_DOCUMENT_BYTES)?;
    if !pdf.starts_with(b"%PDF-") {
        return Err("Документ для печати — не PDF".to_string());
    }
    known_printer(printer, &printers()?)?;
    send(OsStr::new(LP), printer, &pdf, copies, media)
}

/// Принтеры по выводу программы `lpstat`: имена — `-e`, принтер по умолчанию — `-d` (его ошибка не мешает
/// списку).
fn printers_from(lpstat: &OsStr) -> Result<Vec<PrinterInfo>, String> {
    let names = match lpstat_output(lpstat, "-e") {
        Ok(out) if out.status.success() => parse_lpstat_e(&String::from_utf8_lossy(&out.stdout)),
        Ok(out) => {
            return Err(format!(
                "Не удалось получить список принтеров: {}",
                output_text(&out)
            ));
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            tracing::info!("lpstat не найден: CUPS не установлен, принтеров нет");
            return Ok(Vec::new());
        }
        Err(e) => return Err(format!("Не удалось получить список принтеров: {e}")),
    };
    let default = match lpstat_output(lpstat, "-d") {
        Ok(out) if out.status.success() => parse_lpstat_d(&String::from_utf8_lossy(&out.stdout)),
        _ => None,
    };
    Ok(names
        .into_iter()
        .map(|name| PrinterInfo {
            is_default: default.as_deref() == Some(name.as_str()),
            name,
        })
        .collect())
}

/// Запуск `lpstat <arg>` в локали C. CUPS в macOS берёт язык из настроек системы и слушает `LANG`, только если
/// задана `SOFTWARE` (`cupsLangGet` в `cups/language.c`): без неё `lpstat -d` отвечает на языке системы.
fn lpstat_output(lpstat: &OsStr, arg: &str) -> io::Result<Output> {
    Command::new(lpstat)
        .arg(arg)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .env("SOFTWARE", "MyWork")
        .stdin(Stdio::null())
        .output()
}

/// Имена принтеров из вывода `lpstat -e`: по одному в строке (в имени CUPS нет пробелов).
pub(super) fn parse_lpstat_e(out: &str) -> Vec<String> {
    out.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(String::from)
        .collect()
}

/// Принтер по умолчанию из вывода `lpstat -d` (локаль C): «system default destination: X»; «no system default
/// destination» и всё прочее — `None`.
pub(super) fn parse_lpstat_d(out: &str) -> Option<String> {
    out.lines()
        .find_map(|line| line.strip_prefix("system default destination: "))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(String::from)
}

/// Аргументы `lp`: принтер — одним аргументом после `-d`, копии, размер наклейки (только у наклеек) и файл.
pub(super) fn lp_args(
    printer: &str,
    copies: u32,
    media: Option<(f32, f32)>,
    path: &Path,
) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "-d".into(),
        printer.into(),
        "-n".into(),
        copies.to_string().into(),
    ];
    if let Some((width, height)) = media {
        args.push("-o".into());
        args.push(format!("media=Custom.{width}x{height}mm").into());
    }
    args.push(path.into());
    args
}

/// Размер наклейки: обе стороны в [`MEDIA_MM`] (NaN и бесконечность — вне границ).
fn check_media(media_mm: Option<(f32, f32)>) -> Result<Option<(f32, f32)>, String> {
    match media_mm {
        Some((width, height)) if !(MEDIA_MM.contains(&width) && MEDIA_MM.contains(&height)) => {
            Err(format!(
                "Размер наклейки должен быть от {} до {} мм",
                MEDIA_MM.start(),
                MEDIA_MM.end()
            ))
        }
        media => Ok(media),
    }
}

/// Кладёт PDF во временный файл и отдаёт его программе `lp`; файл удаляется, когда `lp` завершилась (она
/// передаёт его CUPS целиком), — и при ошибке тоже.
fn send(
    lp: &OsStr,
    printer: &str,
    pdf: &[u8],
    copies: u32,
    media: Option<(f32, f32)>,
) -> Result<(), String> {
    let file = TempPdf::create_in(&std::env::temp_dir(), pdf)
        .map_err(|e| format!("Ошибка печати: временный файл не записан: {e}"))?;
    let out = Command::new(lp)
        .args(lp_args(printer, copies, media, file.path()))
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Ошибка печати: lp не запущена: {e}"))?;
    drop(file);
    if out.status.success() {
        Ok(())
    } else {
        // Имя принтера и вывод `lp` — в уведомлении, в журнале — только код.
        tracing::warn!("lp завершилась с ошибкой: {}", out.status);
        Err(format!("Ошибка печати: {}", output_text(&out)))
    }
}

/// Что программа сказала об ошибке: поток ошибок, иначе обычный вывод, иначе код выхода.
fn output_text(out: &Output) -> String {
    [&out.stderr, &out.stdout]
        .into_iter()
        .map(|text| String::from_utf8_lossy(text).trim().to_string())
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| out.status.to_string())
}

/// Временный PDF для `lp`: своё имя на каждый вызов, файл создаётся только новым ([`create_new`]), доступен только
/// владельцу и удаляется при `drop` — на любом пути.
struct TempPdf {
    path: PathBuf,
}

impl TempPdf {
    fn create_in(dir: &Path, pdf: &[u8]) -> io::Result<Self> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        for _ in 0..TEMP_TRIES {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |since| since.subsec_nanos());
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = dir.join(format!(
                "mywork-print-{}-{nanos:09}-{n}.pdf",
                std::process::id()
            ));
            match create_new(&path) {
                Ok(mut file) => {
                    // Сначала страж: недописанный файл тоже удаляется.
                    let temp = Self { path };
                    file.write_all(pdf)?;
                    return Ok(temp);
                }
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "все пробованные имена временного файла заняты",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempPdf {
    fn drop(&mut self) {
        if let Err(e) = std::fs::remove_file(&self.path) {
            tracing::warn!("временный файл печати не удалён: {e}");
        }
    }
}

/// Новый файл только для владельца (`0600`); существующий файл или ссылка на месте имени — ошибка
/// `AlreadyExists` (`create_new` — это `O_CREAT | O_EXCL`), чужое не перезаписывается.
fn create_new(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    options.open(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// Тесты, что пишут и запускают программы, — по одному: в Linux сценарий, который в эту минуту держит
    /// открытым на запись потомок соседнего теста (между `fork` и `exec`), не запускается (`ETXTBSY`).
    fn one_at_a_time() -> std::sync::MutexGuard<'static, ()> {
        static PROGRAMS: std::sync::Mutex<()> = std::sync::Mutex::new(());
        PROGRAMS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Исполняемый сценарий оболочки в `dir` — подмена `lpstat` или `lp`.
    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn printer(name: &str, is_default: bool) -> PrinterInfo {
        PrinterInfo {
            name: name.to_string(),
            is_default,
        }
    }

    /// CUPS не разрешает в имени пробелы, `/` и `#`: строка вывода — одно имя целиком; `/` — экземпляр
    /// (`lpoptions -p имя/экземпляр`), его тоже печатает `lp -d`.
    #[test]
    fn lpstat_e_gives_one_printer_per_line() {
        assert_eq!(
            parse_lpstat_e("Акты_HP\nZebra_ZD220\nНаклейки/58x40\n"),
            vec!["Акты_HP", "Zebra_ZD220", "Наклейки/58x40"]
        );
        assert_eq!(parse_lpstat_e("Принтер\r\n\r\n"), vec!["Принтер"]);
        assert!(parse_lpstat_e("").is_empty());
        assert!(parse_lpstat_e("\n\n").is_empty());
    }

    #[test]
    fn lpstat_d_names_the_default_printer_or_none() {
        assert_eq!(
            parse_lpstat_d("system default destination: Zebra_ZD220\n"),
            Some("Zebra_ZD220".to_string())
        );
        assert_eq!(
            parse_lpstat_d("system default destination: Наклейки/58x40\n"),
            Some("Наклейки/58x40".to_string())
        );
        assert_eq!(parse_lpstat_d("no system default destination\n"), None);
        assert_eq!(parse_lpstat_d(""), None);
        // Переведённый вывод не разбирается: `lpstat` запускается в локали C.
        assert_eq!(
            parse_lpstat_d("назначение системы по умолчанию: Zebra_ZD220\n"),
            None
        );
    }

    #[test]
    fn lp_gets_the_printer_as_one_argument_copies_and_media_only_for_labels() {
        let path = Path::new("/tmp/mywork-print-1.pdf");
        let args = |printer, copies, media| -> Vec<String> {
            lp_args(printer, copies, media, path)
                .into_iter()
                .map(|arg| arg.into_string().unwrap())
                .collect()
        };
        // Имя с пробелом и кириллицей (CUPS таких не создаёт, но и оно — один аргумент, без оболочки).
        assert_eq!(
            args("Офисный принтер; rm -rf ~", 1, None),
            [
                "-d",
                "Офисный принтер; rm -rf ~",
                "-n",
                "1",
                "/tmp/mywork-print-1.pdf"
            ]
        );
        assert_eq!(
            args("Zebra_ZD220", 3, Some((58.0, 40.0))),
            [
                "-d",
                "Zebra_ZD220",
                "-n",
                "3",
                "-o",
                "media=Custom.58x40mm",
                "/tmp/mywork-print-1.pdf"
            ]
        );
        assert_eq!(
            args("Zebra_ZD220", 1, Some((57.5, 30.25)))[5],
            "media=Custom.57.5x30.25mm"
        );
    }

    #[test]
    fn a_label_side_goes_from_15_to_200_mm() {
        assert_eq!(check_media(None), Ok(None));
        assert_eq!(check_media(Some((15.0, 200.0))), Ok(Some((15.0, 200.0))));
        assert_eq!(check_media(Some((58.0, 40.0))), Ok(Some((58.0, 40.0))));
        for media in [
            (14.9, 40.0),
            (58.0, 200.1),
            (0.0, 0.0),
            (-58.0, 40.0),
            (f32::NAN, 40.0),
            (58.0, f32::INFINITY),
        ] {
            assert_eq!(
                check_media(Some(media)),
                Err("Размер наклейки должен быть от 15 до 200 мм".to_string()),
                "{media:?}"
            );
        }
    }

    #[test]
    fn a_temporary_pdf_is_new_private_unique_and_removed_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let first = TempPdf::create_in(dir.path(), b"%PDF-1").unwrap();
        let second = TempPdf::create_in(dir.path(), b"%PDF-2").unwrap();
        assert_ne!(first.path(), second.path());
        for (file, bytes) in [(&first, b"%PDF-1"), (&second, b"%PDF-2")] {
            assert_eq!(file.path().parent(), Some(dir.path()));
            let name = file.path().file_name().unwrap().to_str().unwrap();
            assert!(
                name.starts_with("mywork-print-") && name.ends_with(".pdf"),
                "{name}"
            );
            assert_eq!(std::fs::read(file.path()).unwrap(), bytes);
            let mode = std::fs::metadata(file.path()).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let (first_path, second_path) = (first.path().to_owned(), second.path().to_owned());
        drop(first);
        assert!(!first_path.exists());
        assert!(second_path.exists());
        drop(second);
        assert!(!second_path.exists());
    }

    /// O_EXCL: чужой файл или ссылка на месте имени не перезаписываются и не читаются.
    #[test]
    fn a_temporary_file_is_never_opened_over_an_existing_one() {
        let dir = tempfile::tempdir().unwrap();
        let taken = dir.path().join("taken.pdf");
        std::fs::write(&taken, b"someone else's").unwrap();
        assert_eq!(
            create_new(&taken).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&taken).unwrap(), b"someone else's");
        let target = dir.path().join("target");
        std::fs::write(&target, b"target").unwrap();
        let link = dir.path().join("link.pdf");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert_eq!(
            create_new(&link).unwrap_err().kind(),
            io::ErrorKind::AlreadyExists
        );
        assert_eq!(std::fs::read(&target).unwrap(), b"target");
    }

    /// Подменённая `lp` записывает свои аргументы и копию файла и падает: ошибка — с её выводом, временный
    /// файл после её выхода удалён. При успехе — тоже.
    #[test]
    fn lp_prints_a_temporary_copy_that_is_removed_after_lp_exits() {
        let _serial = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let record = "for arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > \"$(dirname \"$0\")/args.txt\"\n\
                      for last in \"$@\"; do :; done\n\
                      cp \"$last\" \"$(dirname \"$0\")/copy.pdf\"\n";
        let failing = script(
            dir.path(),
            "lp-failing",
            &format!("{record}echo 'lp: The printer or class does not exist.' >&2\nexit 1\n"),
        );
        let pdf = b"%PDF-1.7 label";
        assert_eq!(
            send(
                failing.as_os_str(),
                "Наклейки Zebra",
                pdf,
                2,
                Some((58.0, 40.0))
            ),
            Err("Ошибка печати: lp: The printer or class does not exist.".to_string())
        );
        let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(
            args[..6],
            [
                "-d",
                "Наклейки Zebra",
                "-n",
                "2",
                "-o",
                "media=Custom.58x40mm"
            ]
        );
        assert_eq!(args.len(), 7);
        let printed = Path::new(args[6]);
        assert!(printed.is_absolute());
        assert_eq!(std::fs::read(dir.path().join("copy.pdf")).unwrap(), pdf);
        assert!(!printed.exists(), "временный файл остался после ошибки");

        let succeeding = script(dir.path(), "lp-ok", &format!("{record}exit 0\n"));
        assert_eq!(send(succeeding.as_os_str(), "Акты", pdf, 1, None), Ok(()));
        let args = std::fs::read_to_string(dir.path().join("args.txt")).unwrap();
        let printed = PathBuf::from(args.lines().last().unwrap());
        assert!(!printed.exists(), "временный файл остался после печати");

        // Без вывода — код выхода; программы нет — так и сказано.
        let silent = script(dir.path(), "lp-silent", "exit 3\n");
        let error = send(silent.as_os_str(), "Акты", pdf, 1, None).unwrap_err();
        assert!(error.starts_with("Ошибка печати: "), "{error}");
        assert!(error.contains('3'), "{error}");
        let missing = dir.path().join("no-such-lp");
        let error = send(missing.as_os_str(), "Акты", pdf, 1, None).unwrap_err();
        assert!(
            error.starts_with("Ошибка печати: lp не запущена"),
            "{error}"
        );
    }

    /// Подменённая `lpstat` отвечает, только если её запустили в локали C (и с `SOFTWARE`, иначе CUPS в macOS
    /// берёт язык системы).
    #[test]
    fn printers_come_from_lpstat_in_the_c_locale() {
        let _serial = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let lpstat = script(
            dir.path(),
            "lpstat",
            "[ \"$LC_ALL\" = C ] && [ \"$LANG\" = C ] && [ -n \"${SOFTWARE+set}\" ] || exit 9\n\
             case \"$1\" in\n\
             -e) printf 'Акты_HP\\nZebra_ZD220\\n' ;;\n\
             -d) echo 'system default destination: Zebra_ZD220' ;;\n\
             *) exit 2 ;;\n\
             esac\n",
        );
        assert_eq!(
            printers_from(lpstat.as_os_str()),
            Ok(vec![
                printer("Акты_HP", false),
                printer("Zebra_ZD220", true)
            ])
        );

        // Принтера по умолчанию нет — или `lpstat -d` упал: список всё равно есть.
        let no_default = script(
            dir.path(),
            "lpstat-no-default",
            "case \"$1\" in -e) echo 'Акты_HP' ;; -d) exit 1 ;; esac\n",
        );
        assert_eq!(
            printers_from(no_default.as_os_str()),
            Ok(vec![printer("Акты_HP", false)])
        );

        // Принтеров нет; CUPS не установлен — тоже пустой список.
        let empty = script(dir.path(), "lpstat-empty", "exit 0\n");
        assert_eq!(printers_from(empty.as_os_str()), Ok(vec![]));
        let missing = dir.path().join("no-such-lpstat");
        assert_eq!(printers_from(missing.as_os_str()), Ok(vec![]));

        // `lpstat -e` упал — ошибка с его выводом.
        let failing = script(
            dir.path(),
            "lpstat-failing",
            "echo 'lpstat: Bad file descriptor' >&2\nexit 1\n",
        );
        assert_eq!(
            printers_from(failing.as_os_str()),
            Err("Не удалось получить список принтеров: lpstat: Bad file descriptor".to_string())
        );
    }

    /// Вместо ручной проверки `list_printers` в отладочной сборке: настоящий `lpstat` этого компьютера отвечает
    /// без ошибки (без принтеров — пустой список) и по-английски, хотя язык системы может быть другим. Имена
    /// принтеров в вывод теста не попадают.
    #[test]
    fn the_real_lpstat_of_this_computer_lists_printers_without_error() {
        let _serial = one_at_a_time();
        let listed = printers();
        assert!(listed.is_ok(), "{:?}", listed.err());
        let listed = listed.unwrap_or_default();
        assert!(listed.iter().filter(|p| p.is_default).count() <= 1);
        assert!(listed.iter().all(|p| !p.name.trim().is_empty()));
        match lpstat_output(OsStr::new(LPSTAT), "-d") {
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => panic!("lpstat -d не запущен: {e}"),
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                let english = [
                    "system default destination: ",
                    "no system default destination",
                    "lpstat: error - ",
                ];
                assert!(
                    english.iter().any(|known| text.starts_with(known)),
                    "вывод `lpstat -d` не в локали C: принтер по умолчанию не определится"
                );
            }
        }
    }
}
