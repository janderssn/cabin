//! A built-in magazine reader. Pages are rendered one at a time with
//! `pdftoppm` on a worker thread and cached as JPEG files, so a page is
//! prepared once and reading stays quick on battery.
use eframe::egui::{ColorImage, Context};
use std::{collections::HashSet, path::PathBuf, process::Command, sync::mpsc};

const PDFTOPPM: &str = "/usr/bin/pdftoppm";
/// Pixels across a rendered page: sharp at full width on a 2880-pixel screen.
const RENDER_WIDTH: u32 = 2000;

pub struct Reader {
    pub id: String,
    pub title: String,
    pub path: String,
    pub pages: u32,
    /// Current page, counted from 0.
    pub page: u32,
    /// 0 shows the whole page; 1 and up are reading widths.
    pub zoom: usize,
    pub scroll_top: bool,
    requested: HashSet<u32>,
    jobs: mpsc::Sender<(String, u32, PathBuf)>,
    pub results: mpsc::Receiver<(u32, Option<ColorImage>)>,
}
pub fn cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::library::home().join(".cache"))
        .join("cabin/pages")
}
/// Where a rendered page is kept. The name follows the item, so two
/// magazines never share pages.
pub fn page_file(id: &str, page: u32) -> PathBuf {
    let safe: String = id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    cache_dir().join(format!("{safe}-{:04}.jpg", page + 1))
}
/// The next page index after a move, kept inside the document.
pub fn turn(page: u32, pages: u32, by: i64) -> u32 {
    (page as i64 + by).clamp(0, pages.max(1) as i64 - 1) as u32
}
pub fn page_count(path: &str) -> u32 {
    Command::new("/usr/bin/pdfinfo")
        .arg("--")
        .arg(path)
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("Pages:")?.trim().parse().ok())
        })
        .unwrap_or(1)
}
impl Reader {
    pub fn open(ctx: &Context, id: &str, title: &str, path: &str, pages: u32, page: u32) -> Self {
        let (jobs, work) = mpsc::channel::<(String, u32, PathBuf)>();
        let (done, results) = mpsc::channel();
        let repaint = ctx.clone();
        std::thread::spawn(move || {
            while let Ok((pdf, page, file)) = work.recv() {
                if !file.is_file() {
                    let _ = std::fs::create_dir_all(cache_dir());
                    let number = (page + 1).to_string();
                    // pdftoppm adds the extension itself; the path is a plain argument, never a shell string.
                    let _ = Command::new(PDFTOPPM)
                        .args(["-f", &number, "-l", &number, "-singlefile", "-jpeg"])
                        .args(["-scale-to-x", &RENDER_WIDTH.to_string(), "-scale-to-y", "-1"])
                        .arg("--")
                        .arg(&pdf)
                        .arg(file.with_extension(""))
                        .output();
                }
                let image = image::open(&file).ok().map(|img| {
                    let img = img.to_rgba8();
                    ColorImage::from_rgba_unmultiplied(
                        [img.width() as usize, img.height() as usize],
                        img.as_raw(),
                    )
                });
                if done.send((page, image)).is_err() {
                    break;
                }
                repaint.request_repaint();
            }
        });
        let pages = if pages == 0 { page_count(path) } else { pages };
        Self {
            id: id.into(),
            title: title.into(),
            path: path.into(),
            pages,
            page: page.min(pages.saturating_sub(1)),
            zoom: 1,
            scroll_top: true,
            requested: HashSet::new(),
            jobs,
            results,
        }
    }
    /// Ask for a page once; the worker answers on `results`.
    pub fn request(&mut self, page: u32) {
        if page < self.pages && self.requested.insert(page) {
            let _ = self
                .jobs
                .send((self.path.clone(), page, page_file(&self.id, page)));
        }
    }
    /// Forget pages far from the current one so memory stays small.
    pub fn forget_far(&mut self) -> Vec<u32> {
        let far: Vec<u32> = self
            .requested
            .iter()
            .copied()
            .filter(|p| p.abs_diff(self.page) > 2)
            .collect();
        for p in &far {
            self.requested.remove(p);
        }
        far
    }
    pub fn go(&mut self, by: i64) {
        let next = turn(self.page, self.pages, by);
        if next != self.page {
            self.page = next;
            self.scroll_top = true;
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn page_turns_stay_inside_the_magazine() {
        assert_eq!(turn(0, 10, -1), 0);
        assert_eq!(turn(0, 10, 1), 1);
        assert_eq!(turn(9, 10, 1), 9);
        assert_eq!(turn(3, 10, 100), 9);
        assert_eq!(turn(0, 0, 1), 0);
    }
    #[test]
    fn page_files_are_per_item_and_safe() {
        let file = page_file("mag/../x y", 0);
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(name, "mag____x_y-0001.jpg");
        assert_ne!(page_file("a", 1), page_file("b", 1));
    }
}
