use reqwest::Client;
use async_openai::{Client as OAIClient, config::OpenAIConfig};

#[tokio::main]
async fn main() {
    let client = Client::new();
    match client.get("http://host.docker.internal:8080/v1/models").send().await {
        Ok(resp) => {
            println!("Reqwest Status: {}", resp.status());
            match resp.text().await {
                Ok(text) => println!("Reqwest Body: {}", text),
                Err(e) => println!("Error reading body: {}", e),
            }
        },
        Err(e) => println!("Reqwest Error: {}", e),
    }
    
    let config = OpenAIConfig::new().with_api_base("http://host.docker.internal:8080/v1").with_api_key("none");
    let oai_client = OAIClient::with_config(config);
    match oai_client.models().list().await {
        Ok(models) => println!("OAI Models: {:?}", models),
        Err(e) => println!("OAI Error: {:?}", e),
    }
}
