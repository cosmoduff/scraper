pub mod error;
use std::fs;
use soup::prelude::*;
use std::str::FromStr;
use std::path::PathBuf;
use crate::error::FwPullError;

use regex::Regex;
use futures::prelude::*;
use fantoccini::{Client, Locator};
use serde::{Serialize,Deserialize};

pub fn write_output(fw_info: Vec<Server>, path:PathBuf) -> Result<(), FwPullError>{
    let file = fs::File::create(&path)?;
    serde_json::to_writer_pretty(&file, &fw_info)?;
    Ok(())
}

#[derive(Debug)]
pub enum Vendor {
    Dell,
    Hp,
    Oracle,
}

impl FromStr for Vendor {
    type Err = FwPullError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let vendor = match s.to_lowercase().as_str() {
            "dell" => Vendor::Dell,
            "hp" => Vendor::Hp,
            "oracle" => Vendor::Oracle,
            _ => return Err(FwPullError::VendorParse),
        };

        Ok(vendor)
    }
}

#[derive(Debug, Deserialize)]
pub struct ServerIn {
    #[serde(rename = "Vendor")]
    pub vendor: String,
    #[serde(rename = "Model")]
    pub model: String,
}

#[derive(Debug, Serialize)]
pub struct Server {
    #[serde(rename = "Vendor")]
    pub vendor: String,
    #[serde(rename = "Model")]
    pub model: String,
    #[serde(rename = "Current")]
    pub current: Option<String>,
    #[serde(rename = "Approved")]
    pub approved: Option<String>,
}

pub fn dell_model(model: String) -> String {
    return model.to_lowercase().replace(" ", "-")
}

pub async fn get_dell_bios(client: &mut Client, server: &ServerIn) -> Result<Server, fantoccini::error::CmdError> {
    let re = Regex::new(r"Version (\d+\.\d+(\.\d+)?)").unwrap();
    
    let model = dell_model(server.model.to_string());

    let mut out = Server {
        vendor: server.vendor.clone(),
        model: server.model.clone(),
        current: None,
        approved: None,
    };

    // The longest the url should be for dell is 86 characters this will keep us from reallocating with each push
    let mut url: String = String::with_capacity(86); 
    url.push_str("https://www.dell.com/support/home/us/en/04/product-support/product/");
    url.push_str(&model);
    url.push_str("/drivers");

    // create vars for all the locators we will need
    let os_str = r#"//select[@id='operating-system']"#;
    let naa_str = r#"//option[@value='NAA']"#;
    let ddl_str = r#"//select[@id='ddl-category']"#;
    let bios_str = r#"//option[@value='BI']"#;
    let drop_down_str = "button.details-control";
    let old_ver_link = "a.ml-2";
    let old_ver_table = "//*[@id='oldVersionBody']";

    // got to the page
    client.goto(&url).await?;

    // perform actions on the webpage 
    client.wait_for_find(Locator::XPath(os_str))
        .and_then(|element| element.click())
        .await?;

    client.wait_for_find(Locator::XPath(naa_str))
        .and_then(|element| element.click())
        .await?;

    client.wait_for_find(Locator::XPath(ddl_str))
        .and_then(|element| element.click())
        .await?;

    client.wait_for_find(Locator::XPath(bios_str))
        .and_then(|element| element.click())
        .await?;

    client.execute("window.scrollTo(0, document.body.scrollHeight/2);", vec![]).await?;

    // get the source for parsing 
    let src = client.source().await?;
    let soup = Soup::new(&src);
    
    // look for all the table items
    let td_s = soup.tag("td").find_all();

    for td in td_s {
        if let Some(m) = re.find(&td.text()) {
            let r_match = m.as_str().to_string().to_owned();
            let ver: Vec<&str> = r_match.split(" ").collect();
            out.current = Some(ver[1].to_string());
        };
    }
    
    client.wait_for_find(Locator::Css(drop_down_str))
        .and_then(|element| element.click())
        .await?;

    client.wait_for_find(Locator::Css(old_ver_link))
        .and_then(|element| element.click())
        .await?;

    let mut table = client.wait_for_find(Locator::Css("table.w-100 > tbody:nth-child(2) > tr:nth-child(1) > td:nth-child(1) > a:nth-child(1)"))
        .await?;

    let src = table.html(false).await?;

    // get the source for parsing 
    let soup = Soup::new(&src);

    out.approved = match soup.tag("a").find() {
        Some(l) => Some(l.text()),
        None => None,
    };

    Ok(out)
}

pub async fn get_hp_bios(client: &mut Client, server: &ServerIn) -> Result<Server, fantoccini::error::CmdError> {
    let url = "https://support.hpe.com/hpesc/public/home";

    let mut out = Server {
        vendor: server.vendor.clone(),
        model: server.model.clone(),
        current: None,
        approved: None,
    };

    // got to the page
    client.goto(&url).await?;

    Ok(out)
}
