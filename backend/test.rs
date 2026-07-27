use async_openai::{Client, config::OpenAIConfig};

#[tokio::main]
async fn main() {
    let config = OpenAIConfig::new().with_api_base("http://host.docker.internal:8080").with_api_key("none");
    let client = Client::with_config(config);
    match client.models().list().await {
        Ok(models) => println!("Models: {:?}", models),
        Err(e) => println!("Error: {:?}", e),
    }
}
