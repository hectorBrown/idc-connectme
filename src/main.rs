use anyhow::Result;
use anyhow::anyhow;
use fantoccini::{ClientBuilder, Locator};
use regex::Regex;
use serde_json::json;
use std::env;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};

#[tokio::main]
async fn main() {
    match parse_args(env::args().collect()) {
        Ok(captive_url) => match autoconnect(&captive_url).await {
            Ok(_) => {
                println!("Autoconnect successful.");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("Autoconnect failed: {}", e);
                open::that(&captive_url).unwrap_or_else(|err| {
                    eprintln!("Failed to open portal: {}", err);
                    std::process::exit(1);
                });
                std::process::exit(0);
            }
        },
        Err(e) => {
            eprintln!("Error parsing arguments: {}", e);
            std::process::exit(1);
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<String> {
    match args.len() {
        2 => Ok(args[1].clone()),
        _ => Err(anyhow!(
            "Expected exactly one argument, got {}",
            args.len() - 1
        )),
    }
}

async fn start_webdriver() -> Result<(Child, usize)> {
    let mut child = Command::new("chromedriver")
        .arg("--headless")
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow!(
                "Failed to start firefox. Make sure it is installed and in your PATH: {}",
                e
            )
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or(anyhow!("Could not get stdout for webdriver_process"))?;
    let mut reader = BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        if let Some(port) = parse_port_from_line(&line)? {
            return Ok((child, port));
        }
    }
    Err(anyhow!("Could not find port from Webdriver process"))
}

fn parse_port_from_line(line: &str) -> Result<Option<usize>> {
    let re = Regex::new(r"^ChromeDriver was started successfully on port (\d+)\.$")?;
    if let Some(caps) = re.captures(line) {
        return Ok(Some(
            caps.get(1)
                .ok_or(anyhow!("Couldn't extract port from line {}", line))?
                .as_str()
                .parse()?,
        ));
    } else {
        Ok(None)
    }
}

async fn autoconnect(captive_url: &str) -> Result<()> {
    let (mut webdriver_process, port) = start_webdriver().await?;
    println!("Started WebDriver on port {}", port);
    let webdriver_address = format!("http://localhost:{}", port);

    let client = ClientBuilder::native()
        .capabilities(
            json!({
                "goog:chromeOptions": {
                    "args": ["--headless=new", "--no-sandbox", "--disable-gpu"]
                }
            })
            .as_object()
            .ok_or(anyhow!("Failed to create capabilities"))?
            .clone(),
        )
        .connect(webdriver_address.as_str())
        .await
        .map_err(|e| anyhow!("Failed to create client: {}", e))?;

    client.goto(captive_url).await.map_err(|e| {
        anyhow!(
            "Failed to navigate to captive portal URL {}: {}",
            captive_url,
            e
        )
    })?;
    println!("Navigated to captive portal URL {}", captive_url);

    webdriver_process.kill().await?;
    webdriver_process.wait().await?;

    Ok(())
}
