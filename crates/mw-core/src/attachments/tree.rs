//! Копирование дерева `attachments/` в копию (резервную копию): без черновиков, скрытых имён и
//! символических ссылок, с жёсткими ссылками на файлы прошлой копии.

use std::io::{Error, ErrorKind};
use std::path::Path;

use tokio_util::sync::CancellationToken;

/// Сколько файлов и байт в скопированном дереве (жёсткие ссылки считаются как файлы).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TreeStats {
    pub files: u64,
    pub bytes: u64,
}

/// Верхняя папка черновиков: живёт сутки и в копию не входит.
const DRAFTS_DIR: &str = "drafts";

/// Копирует `src` в `dst`. Пропускает верхнюю папку `drafts`, любые имена с точки и всё, что не
/// файл и не папка (символические ссылки не обходятся). Файл, который есть в `link_from` по тому же
/// относительному пути и с тем же размером, становится жёсткой ссылкой на него; не вышло —
/// обычная копия. Нет `src` — пустой итог. Отмена между файлами — `ErrorKind::Interrupted`.
/// Вызывается из `spawn_blocking`.
pub(crate) fn copy_tree(
    src: &Path,
    dst: &Path,
    link_from: Option<&Path>,
    cancel: &CancellationToken,
) -> std::io::Result<TreeStats> {
    check_cancel(cancel)?;
    match std::fs::symlink_metadata(src) {
        Ok(m) if m.is_dir() => {}
        Ok(_) => return Ok(TreeStats::default()),
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(TreeStats::default()),
        Err(e) => return Err(e),
    }
    let mut stats = TreeStats::default();
    walk(src, dst, link_from, true, cancel, &mut stats)?;
    Ok(stats)
}

fn check_cancel(cancel: &CancellationToken) -> std::io::Result<()> {
    if cancel.is_cancelled() {
        Err(Error::new(ErrorKind::Interrupted, "копирование прервано"))
    } else {
        Ok(())
    }
}

fn walk(
    src: &Path,
    dst: &Path,
    link: Option<&Path>,
    top: bool,
    cancel: &CancellationToken,
    stats: &mut TreeStats,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    let entries = match std::fs::read_dir(src) {
        Ok(entries) => entries,
        // Папку убрали между обходом и чтением: в копию она не попадает.
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e),
    };
    for entry in entries {
        check_cancel(cancel)?;
        let entry = entry?;
        let name = entry.file_name();
        let Some(text) = name.to_str() else { continue };
        if text.starts_with('.') || (top && text == DRAFTS_DIR) {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let from = entry.path();
        let to = dst.join(&name);
        if kind.is_dir() {
            let sub_link = link.map(|l| l.join(&name));
            walk(&from, &to, sub_link.as_deref(), false, cancel, stats)?;
        } else if kind.is_file() {
            let size = match entry.metadata() {
                Ok(m) => m.len(),
                Err(e) if e.kind() == ErrorKind::NotFound => continue,
                Err(e) => return Err(e),
            };
            let linked = link.is_some_and(|l| try_link(&l.join(&name), &to, size));
            if !linked {
                match std::fs::copy(&from, &to) {
                    Ok(_) => {}
                    // Файл удалили после чтения папки: допустимо, в копию он не попадает.
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    Err(e) => return Err(e),
                }
            }
            stats.files += 1;
            stats.bytes += size;
        }
    }
    Ok(())
}

/// Жёсткая ссылка на файл прошлой копии: только настоящий файл того же размера.
fn try_link(previous: &Path, to: &Path, size: u64) -> bool {
    match std::fs::symlink_metadata(previous) {
        Ok(m) if m.is_file() && m.len() == size => std::fs::hard_link(previous, to).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn put(root: &Path, rel: &str, bytes: &[u8]) {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn files_of(root: &Path) -> Vec<String> {
        fn walk(dir: &Path, root: &Path, out: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if entry.file_type().unwrap().is_dir() {
                    walk(&path, root, out);
                } else {
                    out.push(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        let mut out = Vec::new();
        walk(root, root, &mut out);
        out.sort();
        out
    }

    #[test]
    fn copy_tree_skips_drafts_hidden_names_and_links() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        put(&src, "orders/1/a.jpg", b"photo");
        put(&src, "orders/1/a.jpg.thumb.jpg", b"thumb!");
        put(&src, "drafts/k/x.jpg", b"draft");
        put(&src, ".import-1/y.jpg", b"import");
        put(&src, "orders/.DS_Store", b"junk");
        #[cfg(unix)]
        std::os::unix::fs::symlink(src.join("orders/1/a.jpg"), src.join("orders/1/l")).unwrap();

        let stats = copy_tree(&src, &dst, None, &CancellationToken::new()).unwrap();

        assert_eq!(
            files_of(&dst),
            ["orders/1/a.jpg", "orders/1/a.jpg.thumb.jpg"]
        );
        assert_eq!(std::fs::read(dst.join("orders/1/a.jpg")).unwrap(), b"photo");
        assert_eq!(
            std::fs::read(dst.join("orders/1/a.jpg.thumb.jpg")).unwrap(),
            b"thumb!"
        );
        assert_eq!(
            stats,
            TreeStats {
                files: 2,
                bytes: 5 + 6
            }
        );
    }

    #[test]
    fn only_the_top_drafts_folder_is_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        put(&src, "orders/1/drafts/a.jpg", b"kept");
        put(&src, "drafts/k/x.jpg", b"skipped");
        let stats = copy_tree(&src, &dst, None, &CancellationToken::new()).unwrap();
        assert_eq!(files_of(&dst), ["orders/1/drafts/a.jpg"]);
        assert_eq!(stats.files, 1);
    }

    #[test]
    fn a_missing_source_gives_an_empty_result() {
        let tmp = tempfile::tempdir().unwrap();
        let stats = copy_tree(
            &tmp.path().join("none"),
            &tmp.path().join("dst"),
            None,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!(stats, TreeStats::default());
    }

    #[cfg(unix)]
    #[test]
    fn copy_tree_links_unchanged_files() {
        use std::os::unix::fs::MetadataExt;
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        let prev = tmp.path().join("prev");
        let dst = tmp.path().join("dst");
        put(&src, "orders/1/a.jpg", b"photo");
        put(&src, "orders/1/b.jpg", b"new-bytes");
        put(&prev, "orders/1/a.jpg", b"photo");
        put(&prev, "orders/1/b.jpg", b"old");

        let stats = copy_tree(&src, &dst, Some(&prev), &CancellationToken::new()).unwrap();

        let a = std::fs::metadata(dst.join("orders/1/a.jpg")).unwrap();
        assert_eq!(a.nlink(), 2, "тот же путь и размер — жёсткая ссылка");
        assert_eq!(std::fs::read(dst.join("orders/1/a.jpg")).unwrap(), b"photo");
        let b = std::fs::metadata(dst.join("orders/1/b.jpg")).unwrap();
        assert_eq!(b.nlink(), 1, "другой размер — обычная копия");
        assert_eq!(
            std::fs::read(dst.join("orders/1/b.jpg")).unwrap(),
            b"new-bytes"
        );
        assert_eq!(stats.files, 2);
        assert_eq!(stats.bytes, 5 + 9);
    }

    #[test]
    fn copy_tree_stops_when_cancelled() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        put(&src, "orders/1/a.jpg", b"photo");
        let token = CancellationToken::new();
        token.cancel();
        let err = copy_tree(&src, &tmp.path().join("dst"), None, &token).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::Interrupted);
    }
}
