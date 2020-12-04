pub mod error;

#[macro_use]
extern crate lazy_static;

use crate::error::FwPullError;
use soup::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use std::thread;

use fantoccini::{Client, Locator};
use futures::prelude::*;
use regex::{Regex, Captures};
use url::Url;
use serde::{Deserialize, Serialize};
use select::document::Document;
use select::predicate::{Predicate, Attr, Class, Name};

pub fn write_output(fw_info: Vec<Server>, path: PathBuf) -> Result<(), FwPullError> {
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
    model.to_lowercase().replace(" ", "-")
}

pub async fn get_dell_bios(
    client: &mut Client,
    server: &ServerIn,
) -> Result<Server, fantoccini::error::CmdError> {
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
    let old_ver_link = "a.pointer-cursor:nth-child(2)";

    // got to the page
    client.goto(&url).await?;

    // perform actions on the webpage
    client
        .wait_for_find(Locator::XPath(os_str))
        .and_then(|element| element.click())
        .await?;

    client
        .wait_for_find(Locator::XPath(naa_str))
        .and_then(|element| element.click())
        .await?;

    client
        .wait_for_find(Locator::XPath(ddl_str))
        .and_then(|element| element.click())
        .await?;

    client
        .wait_for_find(Locator::XPath(bios_str))
        .and_then(|element| element.click())
        .await?;

    //client
    //    .execute("window.scrollTo(0, document.body.scrollHeight/2);", vec![])
    //    .await?;

    // get the source for parsing
    let src = client.source().await?;
    let soup = Soup::new(&src);

    // look for all the table items
    let td_s = soup.tag("td").find_all();

    for td in td_s {
        if let Some(m) = re.find(&td.text()) {
            let r_match = m.as_str().to_string().to_owned();
            let ver: Vec<&str> = r_match.split(' ').collect();
            out.current = Some(ver[1].to_string());
        };
    }

    client
        .wait_for_find(Locator::Css(drop_down_str))
        .and_then(|element| element.click())
        .await?;

    client
        .wait_for_find(Locator::Css(old_ver_link))
        .and_then(|element| element.click())
        .await?;

    let mut table = client
        .wait_for_find(Locator::Css(
            "table.w-100 > tbody:nth-child(2) > tr:nth-child(1) > td:nth-child(1) > a:nth-child(1)",
        ))
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

pub async fn get_hp_bios(client: &mut Client, server: &ServerIn) -> Result<Server, FwPullError> {
    // most time with regex is with compilation.
    // this ensures they only need to be compiled once
    lazy_static! {
        static ref HP_VER: Regex = Regex::new(r"\d+\.\d+").unwrap();
    }

    lazy_static! {
        static ref HP_BIOS_NAME: Regex = Regex::new(r".*System ROM Flash Binary.*").unwrap();
    }

    let url = "https://support.hpe.com/hpesc/public/home";

    let search_box_css = ".magic-box-input > input:nth-child(2)";
    let search_button = ".CoveoSearchButton";
    let bios_search_link = "div.coveo-list-layout:nth-child(1) > div:nth-child(1) > div:nth-child(2) > div:nth-child(2) > div:nth-child(3) > div:nth-child(1) > a:nth-child(1)";
    //let os_independent = "#kmswtargetbaseenvironmentfacet > ul:nth-child(2) > li:nth-child(2) > label:nth-child(1) > div:nth-child(1) > span:nth-child(4)";
    //let date_sort = "#datesort";
    let srv_search_box = "div.CoveoSearchbox:nth-child(2) > div:nth-child(1) > div:nth-child(1) > input:nth-child(2)";
    let srv_search_button = "div.CoveoSearchbox:nth-child(2) > a:nth-child(2) > span:nth-child(1)";
    let first_result = "#driversAndSoftwareTableResultList > table:nth-child(3) > tr:nth-child(2) > td:nth-child(3)";
    let revision_link_str = "#driversAndSoftwareTableResultList > table:nth-child(3) > tr:nth-child(2) > td:nth-child(7) > div:nth-child(1) > a:nth-child(1)";
    let revision_tab = "#ui-id-6";

    let mut out = Server {
        vendor: server.vendor.clone(),
        model: server.model.clone(),
        current: None,
        approved: None,
    };

    // got to the page
    client.goto(&url).await?;

    // put model into the search box
    let mut search_box = client.wait_for_find(Locator::Css(&search_box_css)).await?;
    search_box.send_keys(&server.model).await?;

    // push the search button
    client
        .wait_for_find(Locator::Css(&search_button))
        .and_then(|element| element.click())
        .await?;

    // click link for bios page for top result
    client
        .wait_for_find(Locator::Css(&bios_search_link))
        .and_then(|element| element.click())
        .await?;

    client.wait_for_navigation(None).await?;

    // wait for first result to load
    client
        .wait_for_find(Locator::Css(&revision_link_str))
        .await?;

    let curr_url = client.current_url().await?;
    let new_url = get_hp_date_sort_url(curr_url);

    // for some reason if it doesnt go to a diff url first it never navigates properly to the new url
    client.goto(&url).await?;

    // navigate tot he new url
    if let Some(u) = new_url {
        client.goto(u.as_str()).await?;
    } else {
        return Err(FwPullError::SoupNotFound("Could not navigate to new url".to_string()))
    }

    let result_page_html = client
        .wait_for_find(Locator::Css(&revision_link_str))
        .await?
        .html(false)
        .await?;

    let rev_link_soup = Soup::new(&result_page_html);
    let rev_link_url = match rev_link_soup.tag("a").find() {
        Some(l) => l,
        None => return Err(FwPullError::SoupNotFound("a".to_string())),
    };
    if let Some(l) = rev_link_url.get("href") {
        client.goto(&l).await?;
    } else {
        return Err(FwPullError::SoupNotFound("href.to_string".to_string()));
    }

    client.wait_for_find(Locator::Css(&revision_tab)).await?;

    let src = client.source().await?;
    let soup = Soup::new(&src);

    let bold_tags: Vec<String> = soup.tag("b").find_all().map(|tag| tag.text()).collect();
    for tag in bold_tags {
        if tag.starts_with("Version") {
            if let Some(m) = HP_VER.find(&tag) {
                let fw = m.as_str();
                if out.current.is_none() {
                    out.current = Some(fw.to_string());
                } else if out.current.is_some() && out.approved.is_none() {
                    out.approved = Some(fw.to_string());
                }
            }
        }
    }

    Ok(out)
}

pub async fn get_oracle_bios(server: &ServerIn) -> Result<Server, FwPullError> {
    lazy_static! {
        static ref ORACLE_VER: Regex =
            Regex::new(r"(^Sun System Firmware \d*\.\d*\.\d*(\.[a-z])?)").unwrap();
    }

    let mut out = Server {
        vendor: server.vendor.clone(),
        model: server.model.clone(),
        current: None,
        approved: None,
    };

    let fw_url = "https://www.oracle.com/servers/technologies/firmware/release-history-jsp.html";

    let src = Document::from(reqwest::get(fw_url).await?.text().await?.as_str());

    // find all tr
    let tr_all = src.find(Name("tr"));

    for tr in tr_all {
        if out.current.is_none() {
            out.current = match tr.find(Name("a").and(Attr("id", server.model.as_str()))).next() {
                Some(_) => {
                    tr.find(Name("strong")).next()
                        .and_then(
                            |ver| {
                                if let Some(m) = ORACLE_VER.find(&ver.text()) {
                                    Some(m.as_str().to_string())
                                } else {
                                    None
                                }
                            }
                        )
                },
                None => None,
            };
        } else {
            out.approved = match tr.find(Name("p")).next() {
                Some(p) => {
                    if let Some(m) = ORACLE_VER.find(&p.text()) {
                        Some(m.as_str().to_string())
                    } else {
                        None
                    }
                },
                None => None,
            };
            break
        }
    }
    
    Ok(out)
}

fn get_hp_date_sort_url(url: Url) -> Option<Url> {
    lazy_static! {
        static ref HP_URL: Regex = Regex::new(r"(^t=DriversandSoftware&sort=relevancy&layout=table&numberOfResults=25&f)(.*)").unwrap();
    }

    let mut new_url = url.clone();
    let result = if let Some(f) = url.fragment() {
        HP_URL.replace(f, |caps: &Captures| {format!("t=DriversandSoftware&sort=%40hpescuniversaldate descending&layout=table&numberOfResults=25&f{}",&caps[2])}) 
    } else {
        return None
    };

    new_url.set_fragment(Some(&result));

    Some(new_url)
}
