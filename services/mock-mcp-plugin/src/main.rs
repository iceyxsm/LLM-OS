use std::time::Duration;

fn main() -> anyhow::Result<()> {
    println!("mock-mcp-plugin: started");
    loop {
        std::thread::sleep(Duration::from_secs(30));
    }
}
