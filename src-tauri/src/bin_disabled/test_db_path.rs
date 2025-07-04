use matter_certis_v2_lib::infrastructure::config::ConfigManager;

fn main() {
    println!("Testing database path resolution...");
    
    match ConfigManager::get_app_data_dir() {
        Ok(data_dir) => {
            let db_dir = data_dir.join("database");
            let db_path = db_dir.join("matter_certis.db");
            let database_url = format!("sqlite:{}", db_path.to_string_lossy());
            
            println!("✅ Data directory: {:?}", data_dir);
            println!("✅ DB directory: {:?}", db_dir);
            println!("✅ DB path: {:?}", db_path);
            println!("✅ Database URL: {}", database_url);
            
            println!("Directory exists: {}", db_dir.exists());
            println!("DB file exists: {}", db_path.exists());
        }
        Err(e) => {
            println!("❌ Failed to get data directory: {}", e);
        }
    }
}
