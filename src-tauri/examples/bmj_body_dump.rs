//! What are the 276 characters BMJ returns for every URL?
use app_lib::guidelines::html_to_blocks;
use app_lib::http_fetcher::ReqwestFetcher;
use gaply_core::refverify::{HttpFetcher, HttpRequest};

fn main() {
    let f = ReqwestFetcher::new().unwrap();
    for url in [
        "https://www.bmj.com/about-bmj/resources-authors",
        "https://www.bmj.com/about-bmj/resources-authors/article-types",
        "https://www.bmj.com/about-bmj/resources-authors/house-style",
    ] {
        match f.get(&HttpRequest::get(url)) {
            Err(e) => println!("{url}\n  FETCH ERROR: {e}\n"),
            Ok(r) => {
                let blocks = html_to_blocks(&r.body);
                println!("{url}");
                println!("  raw body chars: {}", r.body.chars().count());
                println!("  blocks: {}", blocks.len());
                for b in blocks.iter().take(2) {
                    println!("  heading={:?}", b.heading);
                    println!("  text  = {:?}", b.text.chars().take(300).collect::<String>());
                }
                println!("  --- first 400 chars of RAW body ---");
                println!("  {:?}\n", r.body.chars().take(400).collect::<String>());
            }
        }
    }
}
