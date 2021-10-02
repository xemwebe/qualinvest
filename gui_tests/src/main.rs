use thirtyfour::prelude::*;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let caps = DesiredCapabilities::firefox();
    let driver = WebDriver::new("http://localhost:4444/wd/hub", &caps).await?;

    // Navigate to URL.
    driver.get("http://127.0.0.1:8000").await?;
    let title = driver.title().await?;
    assert_eq!(title, "QuantInvest - Home");
    println!("Open homepage was successful");

    // login as qltester
    let name = "admin".to_string();
    let password = "admin".to_string();
    driver.get("http://127.0.0.1:8000/login?redirect=").await?;
    driver.find_element(By::Id("username")).await?.send_keys(name).await?;
    driver.find_element(By::Id("password")).await?.send_keys(password).await?;
    driver.find_elements(By::Css("button")).await?[0].click().await?; 
  
    // Get all the elements available with tag name 'p'
    let elements = driver.find_elements(By::Tag("a")).await?;
    let mut found_logout = false;
    for e in elements {
      if e.text().await? =="Logout" {
          found_logout=true;
          break;
      }
    }
    assert!(found_logout);
    println!("Login was successful");
  
    // // Navigate to page, by chaining futures together and awaiting the result.
    // driver.find_element(By::Id("pagetextinput")).await?.click().await?;

    // // Find element.
    // let elem_div = driver.find_element(By::Css("div[data-section='section-input']")).await?;

    // // Find element from element.
    // let elem_text = elem_div.find_element(By::Name("input1")).await?;

    // // Type in the search terms.
    // elem_text.send_keys("selenium").await?;

    // // Click the button.
    // let elem_button = elem_div.find_element(By::Tag("button")).await?;
    // elem_button.click().await?;

    // // Get text value of element.
    // let elem_result = driver.find_element(By::Id("input-result")).await?;
    // assert_eq!(elem_result.text().await?, "selenium");

    // Always explicitly close the browser. There are no async destructors.
    driver.quit().await?;

    Ok(())
}