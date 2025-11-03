use anyhow::Result;
use anyhow::anyhow;
use clap::Parser;
use fantoccini::{ClientBuilder, Locator};
use notify_rust::Notification;
use reqwest::Client as LightClient;
use serde_json::json;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::time::{Duration, sleep};

const SUBMIT_SELECTORS: &[&str] = &["input[type=\"submit\"]"];
const WEBDRIVER_PORT: usize = 41211;
const CONNECTIVITY_CHECK_URL: &str = "http://connectivitycheck.gstatic.com/generate_204";
const CONNECTIVITY_TIMEOUT: usize = 5000;
const CONNECTIVITY_REFRESH: usize = 500;

#[derive(Parser)]
#[command(
    name = "idc-connectme",
    about = "Connect to captive portal networks automatically"
)]
struct Cli {
    #[arg(help = "Captive portal URL to connect to")]
    url: String,

    #[arg(short, long, help = "User to send notification as (for root)")]
    user: Option<String>,
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match autoconnect(&args.url).await {
        Ok(_) => {
            println!("Autoconnect successful.");
            notify(
                "Captive Portal Autoconnect",
                "Successfully connected to network.",
                args.user,
            )
            .unwrap_or_else(|e| {
                eprintln!("Failed to send notification {}", e);
                std::process::exit(1);
            });
            std::process::exit(0);
        }
        Err(e) => {
            eprintln!("Autoconnect failed: {}", e);
            open::that(&args.url).unwrap_or_else(|err| {
                eprintln!("Failed to open portal: {}", err);
                std::process::exit(1);
            });
            std::process::exit(0);
        }
    }
}

async fn check_connected() -> Result<bool> {
    let client = LightClient::new();
    Ok(client.get(CONNECTIVITY_CHECK_URL).send().await?.status()
        == reqwest::StatusCode::from_u16(204)?)
}

fn notify(summary: &str, body: &str, user: Option<String>) -> Result<()> {
    if let Err(err) = Notification::new()
        .summary(summary)
        .body(body)
        .timeout(5000)
        .show()
        .map_err(|e| anyhow!("Error showing notification {}", e))
    {
        if let Some(user) = user {
            let status = std::process::Command::new("systemd-run")
                .arg("--unit=root-notify")
                .arg("--wait")
                .arg(format!("--property=User={}", user))
                .arg("/usr/bin/notify-send")
                .arg(summary)
                .arg(body)
                .status();
            return match status {
                Ok(s) if s.success() => Ok(()),
                Ok(s) => Err(anyhow!(
                    "notify-send failed with status {} after error {}",
                    s,
                    err
                )),
                Err(e) => Err(anyhow!(
                    "Failed to execute notify-send: {} after error {}",
                    e,
                    err
                )),
            };
        } else {
            return Err(anyhow!(
                "Failed to execute notify-send (no user provided) after error {}",
                err
            ));
        }
    }
    Ok(())
}

fn start_webdriver(port: usize) -> Result<Child> {
    Command::new("chromedriver")
        .arg("--headless")
        .arg(format!("--port={}", port))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| {
            anyhow!(
                "Failed to start firefox. Make sure it is installed and in your PATH: {}",
                e
            )
        })
}

async fn autoconnect(captive_url: &str) -> Result<()> {
    let mut webdriver_process = start_webdriver(WEBDRIVER_PORT)?;
    println!("Started WebDriver on port {}", WEBDRIVER_PORT);
    let res = {
        autoconnect_withdriver(
            captive_url,
            format!("http://localhost:{}", WEBDRIVER_PORT).as_str(),
        )
        .await
    };

    webdriver_process.kill().await?;
    webdriver_process.wait().await?;

    res
}

async fn autoconnect_withdriver(captive_url: &str, webdriver_address: &str) -> Result<()> {
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
        .connect(webdriver_address)
        .await
        .map_err(|e| anyhow!("Failed to create client: {}", e))?;

    client.goto(captive_url).await.map_err(|e| {
        anyhow!(
            "Failed to navigate to captive portal URL {}: {}",
            captive_url,
            e
        )
    })?;
    client.wait().for_element(Locator::Css("body")).await?;

    println!("Navigated to captive portal URL {}", captive_url);

    for selector in SUBMIT_SELECTORS {
        println!("Trying selector {}", selector);
        if let Ok(element) = client.find(Locator::Css(selector)).await {
            println!("Found element {}", element.html(false).await?);
            element.click().await.map_err(|e| {
                anyhow!("Failed to click element with selector {}: {}", selector, e)
            })?;
            println!("Clicked element with selector {}", selector);
            break;
        }
    }

    for _ in 0..(CONNECTIVITY_TIMEOUT / CONNECTIVITY_REFRESH) {
        if check_connected().await? {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    Err(anyhow!("Not connected to internet."))
}
