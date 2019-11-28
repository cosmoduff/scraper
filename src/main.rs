use tokio;
use serde_json::{
    json,
    map::Map
};
use fantoccini::{Client, Locator};
use futures_util::future::TryFutureExt;


fn dell_model(model: String) -> String {
    return model.to_lowercase().replace(" ", "-")
}

async fn get_dell_bios(client: &mut Client, model: String) -> Result<(), fantoccini::error::CmdError> {
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

    // got ot the page
    client.goto(&url).await?;

    // walk through abd click the necessary elements
    client.clone().wait_for_find(Locator::XPath(os_str))
        .and_then(|element| element.click())
        .and_then(move |client| client.wait_for_find(Locator::XPath(naa_str)))
        .and_then(|element| element.click())
        .and_then(move |client| client.wait_for_find(Locator::XPath(ddl_str)))
        .and_then(|element| element.click())
        .and_then(move |client| client.wait_for_find(Locator::XPath(bios_str)))
        .and_then(|element| element.click())
        .map_err(|err| panic!("a WebDriver command failed: {:?}", err))
        .await;

    //println!("{}", client.source().await?);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), fantoccini::error::CmdError> {
    //let mut capabilities = Map::new();
    //capabilities.insert("headless".to_string(), json!(true));
    let cap_json = json!({
        "moz:firefoxOptions": {
            "prefs": {
                "headless": true
            }
        }
    });

    println!("{:?}", cap_json.as_object());

    let capabilities = cap_json.as_object().unwrap().to_owned();

    let models = vec!["Poweredge R630", "Poweredge R330", "Poweredge R730", "Poweredge R930"];
    let mut c = Client::with_capabilities("http://localhost:4444", capabilities).await.expect("failed to connect to web driver");
    //let mut c = Client::new("http://localhost:4444").await.expect("failed to connect to web driver");

    for model in models {
        let model = dell_model(model.to_string());
        get_dell_bios(&mut c, model).await?;    
    }


    c.close().await
}
