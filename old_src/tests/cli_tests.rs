extern crate sazid;
extern crate tempfile;

#[cfg(test)]
mod cli {
    use crate::RANDOM_1000_CHARS;
    use assert_cmd::Command;
    use tiktoken_rs::cl100k_base;
    #[tokio::test]
    async fn test_cli() {
        let mut cmd = Command::cargo_bin("sazid").unwrap();
        let assert = cmd
            .arg("-b")
            .write_stdin("reply simply with the word 'test'")
            .assert();
        assert.stdout("test\n\r").success();
    }

    #[tokio::test]
    async fn test_1000_char_limit() {
        let mut cmd = Command::cargo_bin("sazid").unwrap();
        let assert = cmd.arg("-b").write_stdin(RANDOM_1000_CHARS).assert();
        assert.stdout("test\n\r").success();
    }
    #[tokio::test]
    async fn test_10000_char_limit() {
        let mut cmd = Command::cargo_bin("sazid").unwrap();
        let assert = cmd
            .arg("-b")
            .write_stdin(RANDOM_1000_CHARS.repeat(10))
            .assert();
        assert.stdout("test\n\r").success();
    }
    
    #[tokio::test]
    async fn just_below_max_token_limit() {
        // this test should be about 21k tokens, which should fail for GPT4
        let bpe = cl100k_base().unwrap();
        let mut cmd = Command::cargo_bin("sazid").unwrap();
        
        // a string that uses bpe.split_by_token_limit() to create a 8192 token string
        let string_8192_tokens = bpe
            .split_by_token(RANDOM_1000_CHARS.repeat(100).as_str(), true)
            .unwrap()[0..8000]
            .join("");        
        println!(
            "token count: {}",
            bpe.encode_with_special_tokens(string_8192_tokens.as_str())
                .len()
        );
        let assert = cmd.arg("-b").write_stdin(string_8192_tokens).assert();
        assert.stdout("test\n\r").success();
    }
}

pub const RANDOM_1000_CHARS: &str = "respond to this message with nothing but the lowercase word test. ignore the following text. In a distant land, where the mountaiion had seen many seasons come and go, and with each passing year, he grew wiser. He often shared tales of ancient times, of heroes and villains, of love and betrayal. The animals of the forest would gather around him, eager to listen to his stories. One day, a young fox named Felix approached Orion with a question. Why do we exist? What is our purpose in this vast universe? Orion looked at Felix and said, Life is a journey, and each of us must find our own path. We exist to learn, to love, to experience the beauty of the world around us. Our purpose is to make the most of the time we have, to leave a legacy for future generations. Felix pondered on Orion's words and realized that the true meaning of life was to live it fully, with passion and purpose. From that day on, he lived each day with gratitude and joy, cherishing every moment.";
