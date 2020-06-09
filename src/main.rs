mod error;

use std::fs;
use std::str::FromStr;
use std::path::PathBuf;

use crate::error::FwPullError;
use scraper::{
    Server,
    ServerIn,
    Vendor,
    get_dell_bios,
    write_output,
};

use serde_json::json;
use structopt::StructOpt;
use fantoccini::Client;

fn main() -> Result<(), FwPullError> {
    let args = Args::from_args();
    
    let models = match fs::read_to_string(&args.input) {
        Ok(j) => j,
        Err(e) => {
            eprintln!("An error occured while trying to open {}: {}", args.input.to_string_lossy(), e);
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

    smol::run( async {
        // create the capabilities object to set the options for the webdriver
        let cap_json = json!({
            "moz:firefoxOptions": {
                "args": ["--headless"]
            }
        });
        let capabilities = cap_json.as_object().unwrap().to_owned();

        let mut c = match args.debug {
            true => Client::new("http://localhost:4444").await.expect("failed to connect to web driver"),
            false => Client::with_capabilities("http://localhost:4444", capabilities).await.expect("failed to connect to web driver"),
        };

        let mut out: Vec<Server> = Vec::with_capacity(models.len());

        for model in models {
            match Vendor::from_str(&model.vendor) {
                Ok(v) => {
                    match v {
                        Vendor::Dell => {
                            let fw_info = get_dell_bios(&mut c, &model).await?;
                            out.push(fw_info);
                        },
                        _ => eprintln!("Unimplemented vendor")
                    }
                },
                Err(e) => eprintln!("{}", e),
            }
        }

        match args.output {
            Some(p) => {
                if let Err(e) = write_output(out, p) {
                    eprintln!("Failed to write json file: {}", e);
                }
            },
            None => println!("{:?}", out),
        }

        match c.close().await {
            Ok(o) => Ok(o),
            Err(e) => Err(FwPullError::from(e)),
        }
    })
}

#[derive(Debug, StructOpt)]
#[structopt(name = "fw_pull", about = "Pulls vendor firmware from their websites")]
struct Args{
    /// Path to input JSON with server information
    #[structopt(short, long, parse(from_os_str))]
    input: PathBuf,
    /// Path to output JSON
    #[structopt(short, long, parse(from_os_str))]
    output: Option<PathBuf>,
    /// Turns on debug mode and runs the browser in the foreground
    #[structopt(short, long)]
    debug: bool
}
