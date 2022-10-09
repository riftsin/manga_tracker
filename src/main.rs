use clap::{Parser, Subcommand};
use crabquery::Document;
use directories::ProjectDirs;
use rusqlite::{Connection, Result};
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::io::Write;
use temp_file::TempFile;

fn main() -> Result<(), ureq::Error> {
    let cli = Cli::parse();

    match &cli.command {
        Some(_) => {}
        None => {
            let db = copy_firefox_db();
            let conn = Connection::open(db.path()).expect("Couldn't open firefox DB");
            let sql = "SELECT url FROM moz_places WHERE url LIKE 'https://mangahub.io/chapter/%/chapter-%' ORDER BY url DESC";
            let mut stmt = conn
                .prepare(sql)
                .expect(format!("Error in preparing the following query : {}", sql).as_str());
            let mut rows = stmt
                .query([])
                .expect(format!("Failed to execute query : {}", sql).as_str());
            let mut history = HashMap::new();
            while let Some(row) = rows.next().unwrap() {
                let url: String = row.get(0).unwrap();
                let chapter = Chapter::new(url);
                if let Some(v) = history.get(&chapter.url) {
                    if chapter.chapter_number < *v {
                        continue;
                    }
                }
                history.insert(chapter.url, chapter.chapter_number);
            }
            let manga_db = Database::new();
            let blacklist = manga_db.get_blacklist();
            for url in blacklist.iter() {
                history.remove(url);
            }
            let mut new_manga_list: HashSet<String> = history.keys().cloned().collect();
            let whitelist = manga_db.get_whitelist();
            for url in whitelist.iter() {
                new_manga_list.remove(url);
            }
            'outer: for url in new_manga_list.iter() {
                loop {
                    print!("Do  you want to track {} ? [y/n]: ", url);
                    io::stdout().flush().unwrap();
                    let mut answer = String::new();
                    match io::stdin().read_line(&mut answer) {
                        Ok(0) => break 'outer,
                        Ok(_) => match answer.trim() {
                            "y" => {
                                manga_db.add_whitelist(url);
                            }
                            "n" => {
                                manga_db.add_blacklist(url);
                                history.remove(url);
                            }
                            _ => {
                                println!("Please either answer with 'y' or 'n'");
                                continue;
                            }
                        },
                        Err(e) => eprintln!("{}", e),
                    }
                    break;
                }
            }
            for (url, chapter_id) in history.iter() {
                match MangaPage::get(url) {
                    Ok(manga) => {
                        let last_available_chapter = manga.last_chapter();
                        let last_read_chapter = Chapter::from(url, &chapter_id.number);
                        if last_read_chapter.chapter_number < last_available_chapter.chapter_number
                        {
                            // For now just print the last read chapter
                            println!(
                                "{:100}\t\t\tlast chapter {}",
                                last_read_chapter.chapter_url(),
                                last_available_chapter.chapter_number.number
                            );
                        }
                    }
                    Err(e) => eprintln!("{}", e),
                }
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

    fn last_chapter(&self) -> Chapter {
        let sel = self
            .document
            .select("div.tab-content > div > ul > li > span > a");
        let el = sel
            .first()
            .expect(format!("Couldn't grab last chapter from {}", self.url).as_str());
        Chapter::new(el.attr("href").unwrap())
    }
}

#[derive(Parser)]
struct Cli {
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {}

fn copy_firefox_db() -> TempFile {
    let tmp = TempFile::new().expect("Couldn't create temporary file");
    fs::copy("/Users/matt/Library/Application Support/Firefox/Profiles/bvvyy1ja.default-release/places.sqlite", tmp.path()).expect("Couldn't copy firefox db to a temporary file");
    tmp
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

    fn from(url: &str, chapter_id: &str) -> Self {
        Self::new(Self::url(url, chapter_id))
    }

    fn url(url: &str, chapter_id: &str) -> String {
        let url = String::from(url) + "chapter-" + chapter_id;
        url
    }

    fn chapter_url(&self) -> String {
        Self::url(&self.url, &self.chapter_number.number)
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

struct Database {
    conn: Connection,
}

impl Database {
    fn new() -> Self {
        let dir_path = ProjectDirs::from("", "", "Manga_tracker").unwrap();
        if !dir_path.data_dir().exists() {
            fs::create_dir(dir_path.data_dir()).unwrap();
        }
        let db_path = dir_path.data_dir().join("db");
        let conn = Connection::open(db_path).unwrap();
        let sql = "CREATE TABLE IF NOT EXISTS Whitelist(url LONGVARCHAR PRIMARY KEY)";
        conn.execute(sql, []).unwrap();
        let sql = "CREATE TABLE IF NOT EXISTS Blacklist(url LONGVARCHAR PRIMARY KEY)";
        conn.execute(sql, []).unwrap();
        Database { conn }
    }

    fn get_blacklist(&self) -> Vec<String> {
        let mut stmt = self.conn.prepare("SELECT url FROM Blacklist").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut list = vec![];
        while let Some(row) = rows.next().unwrap() {
            let url: String = row.get(0).unwrap();
            list.push(url);
        }
        list
    }

    fn get_whitelist(&self) -> Vec<String> {
        let mut stmt = self.conn.prepare("SELECT url FROM Whitelist").unwrap();
        let mut rows = stmt.query([]).unwrap();
        let mut list = vec![];
        while let Some(row) = rows.next().unwrap() {
            let url = row.get(0).unwrap();
            list.push(url);
        }
        list
    }

    fn add_whitelist(&self, url: &str) {
        let mut stmt = self
            .conn
            .prepare("INSERT INTO Whitelist(url) VALUES(?)")
            .unwrap();
        stmt.execute([url]).unwrap();
    }

    fn add_blacklist(&self, url: &str) {
        let mut stmt = self
            .conn
            .prepare("INSERT INTO Blacklist(url) VALUES(?)")
            .unwrap();
        stmt.execute([url]).unwrap();
    }
}
