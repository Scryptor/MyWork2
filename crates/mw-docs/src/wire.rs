//! Протокол процесса-рендерера: кадры и сообщения между родителем (сервер, программа для
//! компьютера) и рендерером ([`crate::worker`]) через stdin/stdout рендерера.
//!
//! **Кадр** — `u32` длины (little-endian) и столько же байтов. Кадр длиннее [`MAX_FRAME`] (256 МиБ) —
//! ошибка протокола: она видна по длине, память под заявленные байты не выделяется.
//!
//! **Сообщение** — кадр JSON и объявленные в нём двоичные кадры. JSON — внешний тег serde: вариант без
//! полей — строка (`"Ready"`), с полями — объект с одним ключом (`{"Compiled":{"index":0}}`).
//!
//! **Обмен:**
//! 1. Родитель: `{"Hello":{"fonts":{"dirs":["/папка"],"system":true}}}` ([`FontSources`]).
//!    Рендерер собирает книгу шрифтов и отвечает `"Ready"`.
//! 2. Родитель: `{"Run":{"job":{…},"logo":true,"deadline_ms":25000}}`:
//!    - `job` — [`Job`] в JSON, без логотипа (у задания и у документов он не сериализуется);
//!    - при `logo` за сообщением идёт один двоичный кадр — логотип (PNG), один на всё задание;
//!    - `deadline_ms` — срок задания: рендерер, у которого задание идёт дольше `deadline_ms` + 5 с,
//!      выходит сам с кодом 3.
//!
//!    Рендерер отвечает `{"Compiled":{"index":0}}` после каждого документа, затем одним из двух:
//!    - `{"Done":{"page_count":2,"page_size_mm":[58.0,40.0],"format":"Png","frames":2}}` и `frames`
//!      двоичных кадров: при `"format":"Pdf"` — один кадр PDF, при `"Png"` — по кадру PNG на страницу;
//!    - `{"Failed":{"error":…}}` — [`DocsError`] как [`WireError`]: `{"Code":[{"place":{"block":[0],
//!      "line":1,"column":2},"message":"…","hints":[]}]}`, `{"TooManyPages":3}`, `"TooSlow"`…
//! 3. Шаг 2 повторяется. Конец ввода между сообщениями — родитель закрылся: рендерер выходит с кодом 0.
//!
//! Конец ввода посреди кадра или сообщения, длинный кадр, не тот JSON, не то сообщение или не тот
//! порядок — ошибка протокола: рендерер выходит с кодом 2. Числа — числа JSON: десятичные дроби вроде
//! полей шаблона переходят точно, а вычисленный размер страницы может отличаться в последнем бите.
//!
//! [`FontSources`]: crate::FontSources
//! [`DocsError`]: crate::DocsError

use std::borrow::Cow;
use std::io::{self, Read, Write};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::fonts::FontSources;
use crate::job::Job;
use crate::render::WireError;

/// Самый длинный кадр, байт.
pub(crate) const MAX_FRAME: u32 = 256 << 20;

/// Сообщение родителя.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum Request<'a> {
    /// Первое сообщение: откуда брать запасные шрифты.
    Hello { fonts: Cow<'a, FontSources> },
    /// Задание; при `logo` за ним — кадр логотипа.
    Run {
        job: Cow<'a, Job>,
        logo: bool,
        deadline_ms: u64,
    },
}

/// Сообщение рендерера.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) enum Reply {
    /// Книга шрифтов собрана, можно слать задания.
    Ready,
    /// Документ `index` задания скомпилирован.
    Compiled { index: usize },
    /// Задание построено; за сообщением — `frames` двоичных кадров.
    Done {
        page_count: usize,
        /// Размер первой страницы, мм (ширина, высота).
        page_size_mm: (f64, f64),
        format: Format,
        frames: usize,
    },
    /// Задание не построено.
    Failed { error: WireError },
}

/// Что в двоичных кадрах `Done`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Format {
    /// Один кадр PDF.
    Pdf,
    /// Кадр PNG на каждую страницу.
    Png,
}

/// Ошибка протокола (`InvalidData`).
pub(crate) fn protocol(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Пишет кадр. Длиннее [`MAX_FRAME`] — `InvalidInput`, ничего не записано.
pub(crate) fn write_frame(output: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    let len = u32::try_from(bytes.len())
        .ok()
        .filter(|&len| len <= MAX_FRAME)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("кадр {} байт длиннее предела", bytes.len()),
            )
        })?;
    output.write_all(&len.to_le_bytes())?;
    output.write_all(bytes)
}

/// Читает кадр; `None` — ввод кончился до его начала. Конец посреди кадра — `UnexpectedEof`;
/// длина больше [`MAX_FRAME`] — `InvalidData` до чтения байтов.
pub(crate) fn read_frame(input: &mut impl Read) -> io::Result<Option<Vec<u8>>> {
    let mut len = [0; 4];
    let mut got = 0;
    while got < len.len() {
        match input.read(&mut len[got..]) {
            Ok(0) if got == 0 => return Ok(None),
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => got += n,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let len = u32::from_le_bytes(len);
    if len > MAX_FRAME {
        return Err(protocol(format!("кадр {len} байт длиннее предела")));
    }
    // Буфер растёт по мере чтения: короткий ввод с большой длиной не занимает память впустую.
    let mut bytes = Vec::with_capacity(len.min(64 << 10) as usize);
    input
        .by_ref()
        .take(u64::from(len))
        .read_to_end(&mut bytes)?;
    if bytes.len() < len as usize {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok(Some(bytes))
}

/// Пишет сообщение — кадр JSON `message`, за ним кадры `frames` — и сбрасывает вывод.
pub(crate) fn send(
    output: &mut impl Write,
    message: &impl Serialize,
    frames: &[&[u8]],
) -> io::Result<()> {
    let json = serde_json::to_vec(message).map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("сообщение не записано: {}", json_error(&e)),
        )
    })?;
    write_frame(output, &json)?;
    for frame in frames {
        write_frame(output, frame)?;
    }
    output.flush()
}

/// Читает кадр JSON следующего сообщения; `None` — ввод кончился между сообщениями. Объявленные в
/// сообщении двоичные кадры читает вызывающий ([`read_frame`]).
pub(crate) fn receive<T: DeserializeOwned>(input: &mut impl Read) -> io::Result<Option<T>> {
    let Some(json) = read_frame(input)? else {
        return Ok(None);
    };
    serde_json::from_slice(&json)
        .map(Some)
        .map_err(|e| protocol(format!("сообщение не разобрано: {}", json_error(&e))))
}

/// Ошибка JSON для журнала: вид и место, без её текста — serde_json цитирует в нём значения
/// (`invalid type: string "…"`), а это могут быть данные заказа.
fn json_error(error: &serde_json::Error) -> String {
    format!(
        "{:?}, строка {}, знак {}",
        error.classify(),
        error.line(),
        error.column()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Текст ошибки протокола уходит в журнал родителя и в stderr рендерера: значений из сообщения
    /// в нём нет (serde_json пишет `invalid type: string "…"`), только вид ошибки и место.
    #[test]
    fn a_message_that_does_not_parse_is_not_quoted() {
        let mut pipe = Vec::new();
        let written = write_frame(
            &mut pipe,
            r#"{"Compiled":{"index":"Иван Секретов"}}"#.as_bytes(),
        );
        assert!(written.is_ok(), "{written:?}");
        let error = match receive::<Reply>(&mut pipe.as_slice()) {
            Err(error) => error,
            Ok(_) => panic!("ждали ошибку протокола"),
        };
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
        let text = error.to_string();
        assert!(!text.contains("Секретов"), "{text}");
        assert!(text.contains("Data, строка 1, знак"), "{text}");
    }
}
