//! Пул процессов-рендереров со стороны родителя: задания шаблонов со своим Typst-кодом строятся в
//! отдельных процессах ([`crate::worker`]), чтобы бесконечный цикл автора можно было оборвать
//! убийством процесса. Обмен — по протоколу из `wire`.
//!
//! Процесс запускается по первому заданию (или заранее — [`RenderPool::warm`]) и после ответа
//! возвращается к простаивающим. Процесс, у которого вышел срок, упавший или с ошибкой ввода-вывода,
//! убивается и в пул не возвращается; замена ему запускается заранее, в фоне.

use std::borrow::Cow;
use std::io::{self, BufReader, BufWriter, Read};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
#[cfg(feature = "testing")]
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use crate::fonts::FontSources;
use crate::job::{Job, JobBytes, JobResult};
use crate::render::DocsError;
use crate::wire::{self, Format, Reply, Request};
use crate::worker::WORKER_ARG;

/// Сколько готовых процессов держать без дела; лишний после ответа закрывается.
const MAX_IDLE_WORKERS: usize = 2;

/// Запас общего срока задания сверх его бюджета (`JobLimits::budget`): рендерер сам отвечает
/// `TooSlow` после документа, вышедшего за бюджет, а убивается, только если не ответил и так.
const BUDGET_GRACE: Duration = Duration::from_secs(1);

/// `CREATE_NO_WINDOW`: у процесса-рендерера на Windows нет консольного окна.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Пределы времени процесса-рендерера.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkerLimits {
    /// Запуск: от `Hello` до `Ready` (книга шрифтов). Не входит в сроки заданий.
    pub startup: Duration,
    /// Компиляция одного документа: от отправки задания или от готовности предыдущего документа.
    pub per_document: Duration,
}

/// Пул процессов-рендереров: программа `program`, запущенная с [`WORKER_ARG`]. Клоны делят один
/// пул; пул можно звать из нескольких потоков сразу — каждому заданию свой процесс.
#[derive(Clone)]
pub struct RenderPool {
    inner: Arc<Inner>,
}

struct Inner {
    program: PathBuf,
    fonts: FontSources,
    limits: WorkerLimits,
    idle: Mutex<Vec<Worker>>,
    /// Идёт замена убитого процесса ([`RenderPool::rewarm`]): не больше одной за раз.
    rewarming: AtomicBool,
    #[cfg(feature = "testing")]
    started: AtomicUsize,
    /// Поток последней замены — чтобы тест её дождался.
    #[cfg(feature = "testing")]
    rewarm_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl RenderPool {
    /// Пул без процессов: первый запустится по первому заданию или [`RenderPool::warm`]. Запасные
    /// шрифты `fonts` каждый процесс собирает сам при запуске.
    pub fn new(program: PathBuf, fonts: FontSources, limits: WorkerLimits) -> Self {
        Self {
            inner: Arc::new(Inner {
                program,
                fonts,
                limits,
                idle: Mutex::new(Vec::new()),
                rewarming: AtomicBool::new(false),
                #[cfg(feature = "testing")]
                started: AtomicUsize::new(0),
                #[cfg(feature = "testing")]
                rewarm_thread: Mutex::new(None),
            }),
        }
    }

    /// Строит `job` в процессе-рендерере. Блокирующий вызов (зовётся из `spawn_blocking`).
    ///
    /// Ошибки задания (`Code`, `TooManyPages`, `TooSlow` после документа…) приходят от рендерера как
    /// есть, процесс остаётся в пуле. Свои ошибки пула — процесс при них убит:
    /// - [`DocsError::WorkerUnavailable`] — процесс не запустился или не стал готов за
    ///   `startup`;
    /// - [`DocsError::CodeTimeout`] — документ компилировался дольше `per_document`;
    /// - [`DocsError::TooSlow`] — задание шло дольше своего бюджета и ещё секунды;
    /// - [`DocsError::WorkerCrashed`] — процесс закрылся или сломал протокол посреди задания.
    ///
    /// Вместо убитого процесса в фоне сразу запускается новый ([`RenderPool::rewarm`]).
    pub fn run(&self, job: &Job) -> Result<JobResult, DocsError> {
        let mut worker = match self.idle_worker() {
            Some(worker) => worker,
            None => self.start()?,
        };
        match worker.run(job, self.inner.limits.per_document) {
            Outcome::Answered(answer) => {
                self.give_back(worker);
                answer
            }
            // Процесс уже остановлен (stdin, kill, wait) и в пул не возвращается.
            Outcome::Broken(error) => {
                self.rewarm();
                Err(error)
            }
        }
    }

    /// Запускает один процесс и держит его готовым. Блокирующий вызов: ждёт готовности процесса до
    /// `startup`. Неудача — `tracing::warn!`: задание попробует запустить процесс ещё раз.
    pub fn warm(&self) {
        match self.start() {
            Ok(worker) => self.give_back(worker),
            Err(error) => tracing::warn!("процесс-рендерер заранее не запущен: {error}"),
        }
    }

    /// Сколько процессов запущено за жизнь пула. Только с фичей `testing`.
    #[cfg(feature = "testing")]
    pub fn started_for_tests(&self) -> usize {
        self.inner.started.load(Ordering::SeqCst)
    }

    /// Ждёт конца последней замены убитого процесса, если она была. Только с фичей `testing`.
    #[cfg(feature = "testing")]
    pub fn wait_rewarm_for_tests(&self) {
        let thread = self
            .inner
            .rewarm_thread
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(thread) = thread {
            assert!(thread.join().is_ok(), "поток замены процесса упал");
        }
    }

    /// Заменяет убитый процесс заранее: следующее задание (печать сохранённого шаблона, где код
    /// снова выйдет за срок) не ждёт холодного запуска внутри разрешения рендера. Процесс
    /// запускается [`RenderPool::warm`] в фоновом потоке — только если готовых простаивающих нет
    /// (закрывшиеся убираются отсюда) и замена ещё не идёт. Поток держит пул, пока процесс запускается (до `startup`), и отдаёт его
    /// простаивающим; пул, отпущенный всеми, закрывает его.
    fn rewarm(&self) {
        let (none_ready, closed) = {
            let mut idle = self.idle();
            let mut closed = Vec::new();
            for mut worker in std::mem::take(&mut *idle) {
                if worker.is_ready() {
                    idle.push(worker);
                } else {
                    closed.push(worker);
                }
            }
            (idle.is_empty(), closed)
        };
        drop(closed);
        if !none_ready || self.inner.rewarming.swap(true, Ordering::SeqCst) {
            return;
        }
        let pool = self.clone();
        let spawned = std::thread::Builder::new()
            .name("mw-render-rewarm".to_owned())
            .spawn(move || {
                let _guard = RewarmGuard(&pool.inner.rewarming);
                pool.warm();
            });
        match spawned {
            #[cfg(feature = "testing")]
            Ok(thread) => {
                *self
                    .inner
                    .rewarm_thread
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner) = Some(thread);
            }
            #[cfg(not(feature = "testing"))]
            Ok(_) => {}
            Err(error) => {
                self.inner.rewarming.store(false, Ordering::SeqCst);
                tracing::warn!("процесс-рендерер заранее не запущен: поток не создан: {error}");
            }
        }
    }

    fn idle(&self) -> MutexGuard<'_, Vec<Worker>> {
        self.inner
            .idle
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Готовый процесс из простаивающих; закрывшиеся, пока ждали, отбрасываются.
    fn idle_worker(&self) -> Option<Worker> {
        loop {
            let mut worker = self.idle().pop()?;
            if worker.is_ready() {
                return Some(worker);
            }
        }
    }

    /// Возвращает процесс к простаивающим; лишний закрывается (вне замка).
    fn give_back(&self, worker: Worker) {
        let extra = {
            let mut idle = self.idle();
            if idle.len() < MAX_IDLE_WORKERS {
                idle.push(worker);
                None
            } else {
                Some(worker)
            }
        };
        drop(extra);
    }

    /// Запускает процесс и ждёт `Ready` на `Hello` не дольше `startup`.
    fn start(&self) -> Result<Worker, DocsError> {
        let inner = &self.inner;
        let unavailable = DocsError::WorkerUnavailable;
        let mut command = Command::new(&inner.program);
        command
            .arg(WORKER_ARG)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command
            .spawn()
            .map_err(|e| unavailable(format!("{} не запустился: {e}", inner.program.display())))?;
        #[cfg(feature = "testing")]
        inner.started.fetch_add(1, Ordering::SeqCst);
        let (input, events) = match Worker::attach(&mut child) {
            Ok(parts) => parts,
            Err(e) => {
                // Без потока чтения процесс не нужен.
                let _ = child.kill();
                let _ = child.wait();
                return Err(unavailable(format!("поток чтения не запущен: {e}")));
            }
        };
        let mut worker = Worker {
            child,
            input: Some(input),
            events,
        };
        let hello = Request::Hello {
            fonts: Cow::Borrowed(&inner.fonts),
        };
        if let Err(e) = worker.send(&hello, &[]) {
            return Err(unavailable(format!("Hello не отправлен: {e}")));
        }
        match worker.events.recv_timeout(inner.limits.startup) {
            Ok(Event::Ready) => Ok(worker),
            Ok(Event::Lost(reason)) => Err(unavailable(format!(
                "закрылся до готовности: {reason}{}",
                exit_note(worker.stop())
            ))),
            Ok(_) => Err(unavailable("ответил не Ready на Hello".to_owned())),
            Err(RecvTimeoutError::Timeout) => Err(unavailable(format!(
                "не готов за {} с",
                inner.limits.startup.as_secs_f64()
            ))),
            Err(RecvTimeoutError::Disconnected) => {
                Err(unavailable("закрылся до готовности".to_owned()))
            }
        }
    }
}

/// Чем кончилось задание у процесса.
enum Outcome {
    /// Рендерер ответил `Done` или `Failed`: процесс можно брать дальше.
    Answered(Result<JobResult, DocsError>),
    /// Процесс больше не годится (убит по сроку, упал, сломал протокол).
    Broken(DocsError),
}

/// Что прочитал поток чтения из stdout процесса.
enum Event {
    Ready,
    Compiled(usize),
    Done(JobResult),
    Failed(DocsError),
    /// Вывод кончился или не разобран (причина); после этого поток чтения выходит.
    Lost(String),
}

/// Запущенный процесс-рендерер. При сбросе: закрыть stdin, `kill`, `wait` — зомби не остаётся.
struct Worker {
    child: Child,
    input: Option<BufWriter<ChildStdin>>,
    events: mpsc::Receiver<Event>,
}

impl Worker {
    /// Забирает у `child` stdin и stdout и запускает поток чтения stdout.
    fn attach(child: &mut Child) -> io::Result<(BufWriter<ChildStdin>, mpsc::Receiver<Event>)> {
        let input = child.stdin.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let output = child.stdout.take().ok_or(io::ErrorKind::BrokenPipe)?;
        let (sender, events) = mpsc::channel();
        std::thread::Builder::new()
            .name("mw-render-reader".to_owned())
            .spawn(move || read_events(output, &sender))?;
        Ok((BufWriter::new(input), events))
    }

    fn send(&mut self, request: &Request<'_>, frames: &[&[u8]]) -> io::Result<()> {
        let input = self.input.as_mut().ok_or(io::ErrorKind::BrokenPipe)?;
        wire::send(input, request, frames)
    }

    /// Процесс жив и ничего лишнего не прислал.
    fn is_ready(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
            && matches!(self.events.try_recv(), Err(TryRecvError::Empty))
    }

    /// Отправляет `job` и ждёт ответа в сроки: `per_document` на каждый документ, бюджет задания и
    /// [`BUDGET_GRACE`] на всё. Сборка файла после последнего документа своего кода не выполняет и
    /// идёт только в общий срок.
    fn run(&mut self, job: &Job, per_document: Duration) -> Outcome {
        let total = job.limits.budget.saturating_add(BUDGET_GRACE);
        let run = Request::Run {
            job: Cow::Borrowed(job),
            logo: job.logo.is_some(),
            deadline_ms: u64::try_from(total.as_millis()).unwrap_or(u64::MAX),
        };
        let logo: Vec<&[u8]> = job.logo.as_deref().into_iter().collect();
        if let Err(e) = self.send(&run, &logo) {
            return Outcome::Broken(self.send_failed(&e));
        }
        let sent = Instant::now();
        let overall = sent.checked_add(total);
        let mut document = sent.checked_add(per_document);
        loop {
            // Ближайший срок и чем он кончается; при равенстве — общий.
            let (deadline, expired) = match (document, overall) {
                (Some(document), Some(overall)) if document < overall => {
                    (Some(document), DocsError::CodeTimeout)
                }
                (_, Some(overall)) => (Some(overall), DocsError::TooSlow),
                (document, None) => (document, DocsError::CodeTimeout),
            };
            let event = match deadline {
                Some(deadline) => self
                    .events
                    .recv_timeout(deadline.saturating_duration_since(Instant::now())),
                None => self
                    .events
                    .recv()
                    .map_err(|_| RecvTimeoutError::Disconnected),
            };
            match event {
                Ok(Event::Compiled(index)) => {
                    document = if index.saturating_add(1) < job.docs.len() {
                        Instant::now().checked_add(per_document)
                    } else {
                        None
                    };
                }
                Ok(Event::Done(result)) => return Outcome::Answered(Ok(result)),
                Ok(Event::Failed(error)) => return Outcome::Answered(Err(error)),
                Ok(Event::Ready) => return Outcome::Broken(self.crashed("Ready посреди задания")),
                Ok(Event::Lost(reason)) => return Outcome::Broken(self.crashed(&reason)),
                Err(RecvTimeoutError::Disconnected) => {
                    return Outcome::Broken(self.crashed("поток чтения закрылся"));
                }
                Err(RecvTimeoutError::Timeout) => {
                    self.stop();
                    return Outcome::Broken(expired);
                }
            }
        }
    }

    /// Задание не отправлено: своё задание, которое не уходит в кадр (`InvalidInput`), — процесс
    /// недоступен для него; иначе процесс закрылся.
    fn send_failed(&mut self, error: &io::Error) -> DocsError {
        if error.kind() == io::ErrorKind::InvalidInput {
            self.stop();
            DocsError::WorkerUnavailable(format!("задание не отправлено: {error}"))
        } else {
            self.crashed(&format!("задание не отправлено: {error}"))
        }
    }

    /// Процесс прервался посреди задания: убить, дождаться, записать в журнал.
    fn crashed(&mut self, reason: &str) -> DocsError {
        let status = self.stop();
        tracing::warn!(
            "процесс-рендерер прервался посреди задания: {reason}{}",
            exit_note(status)
        );
        DocsError::WorkerCrashed
    }

    /// Закрывает stdin (недописанное отбрасывается: процесс может не читать), убивает процесс и ждёт
    /// его. Повторный вызов ничего не делает. Код выхода — если процесс его отдал.
    fn stop(&mut self) -> Option<ExitStatus> {
        if let Some(input) = self.input.take() {
            drop(input.into_parts());
        }
        // Вышедший сам процесс `kill` не трогает; ошибка тут ничего не меняет.
        let _ = self.child.kill();
        self.child.wait().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Код выхода для журнала: «, exit status: 3»; пусто, если процесс его не отдал.
fn exit_note(status: Option<ExitStatus>) -> String {
    status.map(|s| format!(", {s}")).unwrap_or_default()
}

/// Поток чтения: разбирает сообщения stdout процесса в канал, пока вывод не кончится, не сломается
/// или канал не закроют. Последнее сообщение — всегда [`Event::Lost`].
fn read_events(output: ChildStdout, events: &mpsc::Sender<Event>) {
    let mut input = BufReader::new(output);
    loop {
        let event = match next_event(&mut input) {
            Ok(Some(event)) => event,
            Ok(None) => Event::Lost("вывод закрыт".to_owned()),
            Err(error) => Event::Lost(format!("вывод не разобран: {error}")),
        };
        let lost = matches!(event, Event::Lost(_));
        if events.send(event).is_err() || lost {
            return;
        }
    }
}

/// Следующее сообщение с его двоичными кадрами; `None` — вывод кончился между сообщениями.
fn next_event(input: &mut impl Read) -> io::Result<Option<Event>> {
    let Some(reply) = wire::receive::<Reply>(input)? else {
        return Ok(None);
    };
    let event = match reply {
        Reply::Ready => Event::Ready,
        Reply::Compiled { index } => Event::Compiled(index),
        Reply::Failed { error } => Event::Failed(error.into()),
        Reply::Done {
            page_count,
            page_size_mm,
            format,
            frames,
        } => {
            // PDF — один кадр, PNG — по кадру на страницу.
            let expected = match format {
                Format::Pdf => 1,
                Format::Png => page_count,
            };
            if frames != expected {
                return Err(wire::protocol(format!(
                    "Done: {frames} кадров {format:?} на {page_count} страниц"
                )));
            }
            let mut pages = Vec::new();
            for _ in 0..frames {
                pages.push(wire::read_frame(input)?.ok_or(io::ErrorKind::UnexpectedEof)?);
            }
            let bytes = match format {
                Format::Pdf => JobBytes::Pdf(pages.into_iter().next().unwrap_or_default()),
                Format::Png => JobBytes::Png(pages),
            };
            Event::Done(JobResult {
                page_count,
                page_size_mm,
                bytes,
            })
        }
    };
    Ok(Some(event))
}

/// Снимает флаг замены, когда поток замены кончил — и при панике внутри `warm`.
struct RewarmGuard<'a>(&'a AtomicBool);

impl Drop for RewarmGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewarm_flag_is_released_even_on_panic() {
        let flag = AtomicBool::new(true);
        drop(RewarmGuard(&flag));
        assert!(!flag.load(Ordering::SeqCst));
        flag.store(true, Ordering::SeqCst);
        let caught = std::panic::catch_unwind(|| {
            let _guard = RewarmGuard(&flag);
            panic!("warm");
        });
        assert!(caught.is_err());
        assert!(!flag.load(Ordering::SeqCst));
    }

    fn event(reply: &Reply, frames: &[&[u8]]) -> io::Result<Option<Event>> {
        let mut pipe = Vec::new();
        wire::send(&mut pipe, reply, frames)?;
        next_event(&mut pipe.as_slice())
    }

    fn done(format: Format, page_count: usize, frames: usize) -> Reply {
        Reply::Done {
            page_count,
            page_size_mm: (58.0, 40.0),
            format,
            frames,
        }
    }

    #[test]
    fn done_brings_its_frames() {
        let pdf = event(&done(Format::Pdf, 2, 1), &[b"%PDF"]);
        assert!(
            matches!(pdf, Ok(Some(Event::Done(JobResult { bytes: JobBytes::Pdf(ref b), page_count: 2, .. }))) if b == b"%PDF")
        );
        let png = event(&done(Format::Png, 2, 2), &[b"a", b"b"]);
        assert!(
            matches!(png, Ok(Some(Event::Done(JobResult { bytes: JobBytes::Png(ref p), .. }))) if p == &[b"a".to_vec(), b"b".to_vec()])
        );
    }

    #[test]
    fn done_with_wrong_frames_is_refused() {
        for (reply, frames) in [
            (done(Format::Pdf, 1, 2), 2),
            (done(Format::Pdf, 1, 0), 0),
            (done(Format::Png, 3, 2), 2),
        ] {
            let sent: Vec<&[u8]> = vec![b"x"; frames];
            let error = event(&reply, &sent).err().map(|e| e.kind());
            assert_eq!(error, Some(io::ErrorKind::InvalidData));
        }
        // Объявленный кадр не пришёл.
        let error = event(&done(Format::Pdf, 1, 1), &[]).err().map(|e| e.kind());
        assert_eq!(error, Some(io::ErrorKind::UnexpectedEof));
    }

    #[test]
    fn end_of_output_between_messages_is_none() {
        assert!(matches!(next_event(&mut io::empty()), Ok(None)));
    }
}
