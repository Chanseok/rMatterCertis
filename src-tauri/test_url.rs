use crate::infrastructure::config::utils;

fn main() {
    println!("Page 1: {}", utils::matter_products_page_url(1));
    println!("Page 2: {}", utils::matter_products_page_url(2));
    println!("Page 464: {}", utils::matter_products_page_url(464));
    println!("Page 480: {}", utils::matter_products_page_url(480));
}
