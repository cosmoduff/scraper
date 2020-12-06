mod error;

use std::fs;
use std::process::{Command, Child, Stdio};
use std::path::PathBuf;
use std::str::FromStr;

use crate::error::FwPullError;
use scraper::{
    get_dell_bios, get_hp_bios, get_oracle_bios, write_output, Server, ServerIn, Vendor,
};

use fantoccini::Client;
use serde_json::json;
use structopt::StructOpt;

#[tokio::main]
async fn main() -> Result<(), FwPullError> {
    let args = Args::from_args();

    let models = match fs::read_to_string(&args.input) {
        Ok(j) => j,
        Err(e) => {
            eprintln!(
                "An error occured while trying to open {}: {}",
                args.input.to_string_lossy(),
                e
            );
            std::process::exit(1);
        }
    };
    let models: Vec<ServerIn> = match serde_json::from_str(&models) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("An error occured while deserializing the input: {}", e);
            std::process::exit(1);
        }
    };

    // create the capabilities object to set the options for the webdriver
    let cap_json = if args.debug == true {
        json!({
            "moz:firefoxOptions": {
                "prefs": {
                    "browser.privatebrowsing.autostart": true
                }
            }
        })
    } else {
        json!({
            "moz:firefoxOptions": {
                "args": ["--headless"],
                "prefs": {
                    "browser.privatebrowsing.autostart": true
                }
            }
        })
    };

    let capabilities = cap_json.as_object().unwrap().to_owned();

    let mut geckodriver: Option<Child> = None;
    let mut c = match Client::with_capabilities("http://localhost:4444", capabilities.clone()).await {
        Ok(client) => client,
        Err(_) => {
            println!("geckodriver is not running. Attempting to start....");
            match Command::new("geckodriver")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn() {
                Ok(child) => {
                    geckodriver = Some(child);
                    println!("started geckodriver");
                    Client::with_capabilities("http://localhost:4444", capabilities).await.expect("unable to connect to the geckodriver. Exitting...")
                },
                Err(_) => {
                    eprintln!("Failed to start geckdriver. Exitting...");
                    std::process::exit(1);
                },
            }
        }
    };

    c.set_window_size(1920, 1080).await?;

    let mut out: Vec<Server> = Vec::with_capacity(models.len());

    for model in models {
        match Vendor::from_str(&model.vendor) {
            Ok(v) => match v {
                Vendor::Dell => match get_dell_bios(&mut c, &model).await {
                    Ok(fw) => out.push(fw),
                    Err(e) => {
                        eprintln!("An error occurred getting Dell firmware: {}", e);
                    }
                },
                Vendor::Hp => match get_hp_bios(&mut c, &model).await {
                    Ok(fw) => out.push(fw),
                    Err(e) => eprintln!("An error occurred getting HP firmware: {}", e),
                },
                Vendor::Oracle => {
                    match get_oracle_bios(&model).await {
                        Ok(fw) => out.push(fw),
                        Err(e) => {
                            eprintln!("An error occurred getting Oracles firmware: {}", e)
                        }
                    };
                }
                _ => {}
            },
            Err(e) => eprintln!("{}", e),
        }
    }

    // write the output to a file or print the output. 
    // if serializing to json fails for printing the struct in debug mode is printed
    match args.output {
        Some(p) => {
            if let Err(e) = write_output(out, p) {
                eprintln!("Failed to write json file: {}", e);
            }
        }
        None => {
            match serde_json::to_string_pretty(&out) {
                Ok(pj) => println!("{}", pj),
                Err(_) => println!("{:?}", out),
            }
        },
    }

    if let Some( mut gd) = geckodriver {
        if let Err(e) = gd.kill() {
            eprintln!("Failed to stop spawned geckodriver driver process: {}", e);
        }
    }

    match c.close().await {
        Ok(o) => Ok(o),
        Err(e) => Err(FwPullError::from(e)),
    }
}

#[derive(Debug, StructOpt)]
#[structopt(name = "fw_pull", about = "Pulls vendor firmware from their websites")]
struct Args {
    /// Path to input JSON with server information
    #[structopt(short, long, parse(from_os_str))]
    input: PathBuf,
    /// Path to output JSON
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,
    /// Turns on debug mode and runs the browser in the foreground
    #[structopt(short, long)]
    debug: bool,
}
