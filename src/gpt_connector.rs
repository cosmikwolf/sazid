use async_openai::{Client, CompletionRequest, EngineId};
use std::env;

pub struct GPTConnector {
    client: Client,
    engine: EngineId,
}

impl GPTConnector {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("OPENAI_API_KEY")?;
        let client = Client::new(api_key).await?;
        let engine = EngineId::Gpt35Turbo; // Default engine
        Ok(GPTConnector { client, engine })
    }

    pub fn set_engine(&mut self, engine_name: &str) {
        self.engine = EngineId::from(engine_name);
    }

    pub async fn send_request(&self, prompt: &str) -> Result<String, Box<dyn std::error::Error>> {
        let completion_request = CompletionRequest::new(prompt);
        let response = self.client.complete(&self.engine, &completion_request).await?;
        Ok(response.choices[0].text.clone())
    }

    pub async fn send_chat_request(&self, messages: Vec<(&str, &str)>) -> Result<String, Box<dyn std::error::Error>> {
        let completion_request = CompletionRequest::new("").messages(messages);
        let response = self.client.complete(&self.engine, &completion_request).await?;
        Ok(response.choices[0].text.clone())
    }

    pub async fn list_engines(&self) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let engines = self.client.list_engines().await?;
        Ok(engines.engines.iter().map(|e| e.id.clone()).collect())
    }

    pub async fn get_engine_info(&self) -> Result<String, Box<dyn std::error::Error>> {
        let engine_info = self.client.get_engine(&self.engine).await?;
        Ok(format!("{:?}", engine_info))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initialization() {
        let connector = GPTConnector::new().await;
        assert!(connector.is_ok());
    }

    #[tokio::test]
    async fn test_engine_setting() {
        let mut connector = GPTConnector::new().await.unwrap();
        connector.set_engine("gpt-3.5-turbo");
        assert_eq!(connector.engine, EngineId::Gpt35Turbo);
    }

    #[tokio::test]
    async fn test_request_sending() {
        let connector = GPTConnector::new().await.unwrap();
        let response = connector.send_request("Hello, world!").await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_chat_request() {
        let connector = GPTConnector::new().await.unwrap();
        let messages = vec![("user", "Hello, world!")];
        let response = connector.send_chat_request(messages).await;
        assert!(response.is_ok());
    }

    // ... other tests
}
