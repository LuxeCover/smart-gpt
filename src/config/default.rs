pub const DEFAULT_CONFIG: &str = r#"
personality: A superintelligent AI.
type: !assistant
    assistant token limit: 400
agents:
    dynamic:
        llm:
            chatgpt:
                api key: PUT YOUR KEY HERE
                model: gpt-3.5-turbo
                embedding model: text-embedding-ada-002
        memory:
            local: {}
    static:
        llm:
            chatgpt:
                api key: PUT YOUR KEY HERE
                model: gpt-3.5-turbo
                embedding model: text-embedding-ada-002
        memory:
            local: {}
    fast:
        llm:
            chatgpt:
                api key: PUT YOUR KEY HERE
                model: gpt-3.5-turbo
                embedding model: text-embedding-ada-002
        memory:
            local: {}
plugins:
    assets: {}
    browse: {}
    google:
        cse id: PUT YOUR CSE ID HERE
        api key: PUT YOUR KEY HERE
    wolfram:
        app id: PUT YOUR APP ID HERE
    newsapi:
        api key: PUT YOUR KEY HERE
    #file system: {}
disabled commands: []
"#;