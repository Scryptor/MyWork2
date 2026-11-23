//! Процесс-рендерер со своей стороны: та же программа, запущенная с [`WORKER_ARG`], строит задания
//! родителя по протоколу из `wire`. В stdout он пишет только кадры и журнала не ведёт.

use std::io::{self, BufReader, BufWriter, Read, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::Duration;

use crate::fonts::FontSet;
use crate::job::{Job, JobBytes, run_job_with_fonts};
use crate::wire::{self, Format, Reply, Request};

/// Первый аргумент программы, который включает режим рендерера ([`worker_main`]).
pub const WORKER_ARG: &str = "--mw-render-worker";

/// Запас сторожа сверх срока задания. Родитель убивает рендерер раньше; сторож нужен, когда родителя
/// нет или он завис.
const WATCHDOG_GRACE: Duration = Duration::from_secs(5);

/// Код выхода рендерера, задание которого шло дольше срока и запаса.
const EXIT_OVERDUE: i32 = 3;

/// Режим рендерера: [`serve`] на stdin/stdout. На время каждого задания работает сторож: задание
/// дольше `deadline_ms` + 5 с завершает процесс с кодом 3 — так рендерер, у которого умер родитель,
/// не крутит чужой цикл часами.
///
/// Код выхода: 0 — stdin закрыт между сообщениями; 2 — ошибка протокола или вывода (о ней — строка в
/// stderr).
pub fn worker_main() -> i32 {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let watchdog = |deadline: Duration| {
        Watchdog::start(deadline.saturating_add(WATCHDOG_GRACE), || {
            std::process::exit(EXIT_OVERDUE)
        })
    };
    match serve_with(stdin.lock(), stdout.lock(), watchdog) {
        Ok(()) => 0,
        Err(error) => {
            // Родителя может уже не быть: ошибка записи сюда ничего не меняет.
            let _ = writeln!(io::stderr(), "процесс-рендерер: ошибка протокола: {error}");
            2
        }
    }
}

/// Отвечает на сообщения родителя из `input` в `output` (протокол — в `wire`): `Hello` — собирает
/// книгу шрифтов и отвечает `Ready`; каждое `Run` — `Compiled` после каждого документа, затем `Done`
/// с байтами или `Failed` с ошибкой.
///
/// Конец ввода между сообщениями — `Ok(())`. Конец посреди кадра или сообщения — `UnexpectedEof`;
/// кадр длиннее 256 МиБ (до выделения памяти под него), не то сообщение или не тот порядок —
/// `InvalidData`; ошибка вывода — как есть.
pub fn serve(input: impl Read, output: impl Write) -> io::Result<()> {
    serve_with(input, output, |_| Ok(()))
}

/// [`serve`], где `guard(срок задания)` зовётся, когда пришло задание; его результат живёт, пока не
/// отправлен ответ на задание.
fn serve_with<G>(
    input: impl Read,
    output: impl Write,
    mut guard: impl FnMut(Duration) -> io::Result<G>,
) -> io::Result<()> {
    let mut input = BufReader::new(input);
    let mut output = BufWriter::new(output);
    let fonts = match wire::receive::<Request>(&mut input)? {
        None => return Ok(()),
        Some(Request::Hello { fonts }) => FontSet::new(&fonts),
        Some(Request::Run { .. }) => return Err(wire::protocol("задание до Hello")),
    };
    wire::send(&mut output, &Reply::Ready, &[])?;
    while let Some(request) = wire::receive::<Request>(&mut input)? {
        let Request::Run {
            job,
            logo,
            deadline_ms,
        } = request
        else {
            return Err(wire::protocol("Hello второй раз"));
        };
        let _guard = guard(Duration::from_millis(deadline_ms))?;
        let mut job = job.into_owned();
        if logo {
            let logo = wire::read_frame(&mut input)?.ok_or(io::ErrorKind::UnexpectedEof)?;
            job.logo = Some(logo.into());
        }
        answer(&job, &fonts, &mut output)?;
    }
    Ok(())
}

/// Строит `job`, сообщая о каждом документе, и отвечает `Done` с байтами или `Failed`.
fn answer(job: &Job, fonts: &FontSet, output: &mut impl Write) -> io::Result<()> {
    // Обратный вызов не прерывает задание: ошибка вывода ждёт его конца.
    let mut sent = Ok(());
    let result = run_job_with_fonts(job, fonts, &mut |index| {
        if sent.is_ok() {
            sent = wire::send(output, &Reply::Compiled { index }, &[]);
        }
    });
    sent?;
    match result {
        Ok(result) => {
            let (format, frames): (_, Vec<&[u8]>) = match &result.bytes {
                JobBytes::Pdf(pdf) => (Format::Pdf, vec![pdf.as_slice()]),
                JobBytes::Png(pages) => (Format::Png, pages.iter().map(Vec::as_slice).collect()),
            };
            let done = Reply::Done {
                page_count: result.page_count,
                page_size_mm: result.page_size_mm,
                format,
                frames: frames.len(),
            };
            wire::send(output, &done, &frames)
        }
        Err(error) => wire::send(
            output,
            &Reply::Failed {
                error: error.into(),
            },
            &[],
        ),
    }
}

/// Сторож задания: по истечении срока зовёт `on_expiry`, если его раньше не отпустили (drop).
struct Watchdog {
    _release: mpsc::Sender<()>,
}

impl Watchdog {
    fn start(limit: Duration, on_expiry: impl FnOnce() + Send + 'static) -> io::Result<Self> {
        let (release, released) = mpsc::channel::<()>();
        std::thread::Builder::new()
            .name("mw-render-watchdog".to_owned())
            .spawn(move || {
                // Отпустили — отправитель закрыт (`Disconnected`); иначе срок вышел.
                if let Err(RecvTimeoutError::Timeout) = released.recv_timeout(limit) {
                    on_expiry();
                }
            })?;
        Ok(Self { _release: release })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn watched(limit: Duration) -> (Watchdog, mpsc::Receiver<()>) {
        let (fired, on_fire) = mpsc::channel();
        let dog = Watchdog::start(limit, move || {
            let _ = fired.send(());
        })
        .unwrap_or_else(|e| panic!("сторож запущен: {e}"));
        (dog, on_fire)
    }

    #[test]
    fn watchdog_fires_after_its_limit() {
        let (_dog, fired) = watched(Duration::from_millis(20));
        assert_eq!(fired.recv_timeout(Duration::from_secs(10)), Ok(()));
    }

    #[test]
    fn released_watchdog_stays_quiet() {
        // Срок длинный: отпущенный сторож закрывает канал сразу, не дожидаясь его.
        let (dog, fired) = watched(Duration::from_secs(60));
        drop(dog);
        assert_eq!(
            fired.recv_timeout(Duration::from_secs(10)),
            Err(RecvTimeoutError::Disconnected)
        );
    }
}
