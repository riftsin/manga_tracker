use clap::{Parser, Subcommand};
use crabquery::Document;
use rusqlite::{Connection, Result};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use temp_file::TempFile;

fn main() -> Result<(), ureq::Error> {
    let cli = Cli::parse();

    match &cli.command {
        Some(Commands::Add { url }) => {
            let manga_page = MangaPage::get(url)?;
            println!("{}", manga_page.last_chapter());
        }
        None => {
            let db = copy_firefox_db();
            let conn = Connection::open(db.path()).expect("Couldn't open firefox DB");
            let sql = "SELECT url FROM moz_places WHERE url LIKE 'https://mangahub.io/chapter/%' ORDER BY url DESC";
            let mut stmt = conn
                .prepare(sql)
                .expect(format!("Error in preparing the following query : {}", sql).as_str());
            let mut rows = stmt
                .query([])
                .expect(format!("Failed to execute query : {}", sql).as_str());
            let mut map = HashMap::new();
            while let Some(row) = rows.next().unwrap() {
                let url: String = row.get(0).unwrap();
                let chapter = Chapter::new(url);
                if let Some(v) = map.get(&chapter.url) {
                    if chapter.chapter_number < *v {
                        continue;
                    }
                }
                map.insert(chapter.url, chapter.chapter_number);
            }
            for (k, v) in map.iter() {
                println!("{}chapter-{}", k, v.number);
            }
        }
    }
    Ok(())
}

struct MangaPage {
    url: String,
    document: Document,
}

impl MangaPage {
    fn get(url: &str) -> Result<Self, ureq::Error> {
        let body: String = ureq::get(url).call()?.into_string()?;
        let document = Document::from(body);
        Ok(Self {
            url: String::from(url),
            document,
        })
    }

    fn last_chapter(&self) -> String {
        let sel = self
            .document
            .select("div.tab-content > div > ul > li > span > a");
        let el = sel.first().unwrap();
        el.attr("href").unwrap()
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Add { url: String },
}

fn copy_firefox_db() -> TempFile {
    let tmp = TempFile::new().expect("Couldn't create temporary file");
    fs::copy("/Users/matt/Library/Application Support/Firefox/Profiles/bvvyy1ja.default-release/places.sqlite", tmp.path()).expect("Couldn't copy firefox db to a temporary file");
    tmp
}

fn get_manga_url(url: &str) -> String {
    if let Some(i) = url.rfind("/") {
        String::from(&url[0..i + 1])
    } else {
        String::new()
    }
}

fn sanatize_url(url: &mut String) {
    if let Some(i) = url.rfind("?reloadKey=1") {
        url.truncate(i);
    }
}

struct Chapter {
    url: String,
    chapter_number: ChapterNumber,
}

impl Chapter {
    fn new(url: String) -> Self {
        let mut url = url;
        sanatize_url(&mut url);
        let chapter_suffix = "chapter-";
        let end_url_index = url.rfind(chapter_suffix).unwrap();
        let number_index = end_url_index + chapter_suffix.len();
        Chapter {
            url: String::from(&url[0..end_url_index]),
            chapter_number: ChapterNumber::new(&url[number_index..url.len()]),
        }
    }
}

struct ChapterNumber {
    number: String,
}

impl ChapterNumber {
    fn new(number: &str) -> Self {
        ChapterNumber {
            number: String::from(number),
        }
    }
}

impl PartialEq for ChapterNumber {
    fn eq(&self, other: &Self) -> bool {
        self.number == other.number
    }
}

impl Ord for ChapterNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.eq(other) {
            return Ordering::Equal;
        }
        let v1: Vec<&str> = self.number.split(".").collect();
        let v2: Vec<&str> = other.number.split(".").collect();
        if v1[0].len() < v2[0].len() {
            return Ordering::Less;
        }
        if v1[0].len() > v2[0].len() {
            return Ordering::Greater;
        }
        if v1[0] < v2[0] {
            return Ordering::Less;
        }
        if v1[0] > v2[0] {
            return Ordering::Greater;
        }
        if v1.len() < v2.len() {
            return Ordering::Less;
        }
        if v1.len() > v2.len() {
            return Ordering::Greater;
        }
        v1[1].cmp(v2[1])
    }
}

impl PartialOrd for ChapterNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for ChapterNumber {}
