//! **Apply pending migrations to a named database, and say what changed.**
//!
//! Run deliberately and only when asked: `Database::open` migrates, so this
//! exists to make that an explicit act with a before/after rather than a side
//! effect of launching something.
use gaply_core::Database;

fn main() {
    let path = std::env::args().nth(1).expect("db path");
    let p = std::path::Path::new(&path);
    println!("opening {path}");
    let db = Database::open(p).expect("open + migrate");
    drop(db);
    println!("migrated (Database::open applies pending migrations)");
}
