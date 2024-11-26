use async_openai::types::CreateEmbeddingRequestArgs;
use async_openai::{types::CreateCompletionRequestArgs, Client};
use futures_util::{Future, StreamExt};
use kalosm_common::*;
use kalosm_streams::text_stream::ChannelTextStream;
use std::pin::Pin;
use std::sync::Arc;
use tokenizers::tokenizer::Tokenizer;

use crate::{Embedder, Embedding, GenerationParameters, ModelBuilder, VectorSpace};

/// A model that uses OpenAI's API.
pub struct RemoteOpenAICompatibleModel {
    model: String,
    client: Client<async_openai::config::OpenAIConfig>,
}

/// A builder for any remote OpenAI compatible model.
#[derive(Debug, Default)]
pub struct RemoteOpenAICompatibleModelBuilder<const WITH_NAME: bool> {
    model: Option<String>,
    config: async_openai::config::OpenAIConfig,
}

impl RemoteOpenAICompatibleModelBuilder<false> {
    /// Creates a new builder
    pub fn new() -> Self {
        Self {
            model: None,
            config: Default::default(),
        }
    }

    /// Set the name of the model to use.
    pub fn with_model(self, model: impl ToString) -> RemoteOpenAICompatibleModelBuilder<true> {
        RemoteOpenAICompatibleModelBuilder {
            model: Some(model.to_string()),
            config: self.config,
        }
    }
}

impl<const WITH_NAME: bool> RemoteOpenAICompatibleModelBuilder<WITH_NAME> {
    /// Sets the API key for the builder.
    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.config = self.config.with_api_key(api_key);
        self
    }

    /// Set the base URL of the API.
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.config = self.config.with_api_base(base_url);
        self
    }

    /// Set the organization ID for the builder.
    pub fn with_organization_id(mut self, organization_id: &str) -> Self {
        self.config = self.config.with_org_id(organization_id);
        self
    }
}

impl RemoteOpenAICompatibleModelBuilder<true> {
    /// Build the model.
    pub fn build(self) -> RemoteOpenAICompatibleModel {
        RemoteOpenAICompatibleModel {
            model: self.model.unwrap(),
            client: Client::with_config(self.config),
        }
    }
}

impl RemoteOpenAICompatibleModel {
    /// Creates a new builder
    pub fn builder() -> RemoteOpenAICompatibleModelBuilder<false> {
        RemoteOpenAICompatibleModelBuilder::new()
    }
}

#[async_trait::async_trait]
impl crate::model::Model for RemoteOpenAICompatibleModel {
    type TextStream = ChannelTextStream;
    type SyncModel = crate::SyncModelNotSupported;

    fn tokenizer(&self) -> Arc<Tokenizer> {
        panic!("OpenAI does not expose tokenization")
    }

    async fn stream_text_inner(
        &self,
        prompt: &str,
        generation_parameters: GenerationParameters,
    ) -> anyhow::Result<Self::TextStream> {
        let mut builder = CreateCompletionRequestArgs::default();
        builder
            .model(&self.model)
            .n(1)
            .prompt(prompt)
            .stream(true)
            .frequency_penalty(generation_parameters.repetition_penalty)
            .temperature(generation_parameters.temperature)
            .max_tokens(generation_parameters.max_length as u16);
        if let Some(stop_on) = generation_parameters.stop_on {
            builder.stop(stop_on);
        }
        let request = builder.build()?;

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        let mut stream = self.client.completions().create_stream(request).await?;

        tokio::spawn(async move {
            while let Some(response) = stream.next().await {
                match response {
                    Ok(response) => {
                        let text = response.choices[0].text.clone();
                        if tx.send(text).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Error in OpenAI stream: {}", e);
                        break;
                    }
                }
            }

            Ok::<(), anyhow::Error>(())
        });

        Ok(rx.into())
    }
}

macro_rules! openai_completion_model {
    ($ty: ident, $tybuilder: ident, $model: literal) => {
        /// A model that uses OpenAI's API.
        pub struct $ty {
            inner: RemoteOpenAICompatibleModel,
        }

        /// A builder for
        #[doc = $model]
        #[derive(Debug, Default)]
        pub struct $tybuilder {
            inner: RemoteOpenAICompatibleModelBuilder<true>,
        }

        impl $tybuilder {
            /// Creates a new builder
            pub fn new() -> Self {
                Self {
                    inner: RemoteOpenAICompatibleModelBuilder::new().with_model($model),
                }
            }

            /// Sets the API key for the builder.
            pub fn with_api_key(mut self, api_key: &str) -> Self {
                self.inner = self.inner.with_api_key(api_key);
                self
            }

            /// Set the base URL of the API.
            pub fn with_base_url(mut self, base_url: &str) -> Self {
                self.inner = self.inner.with_base_url(base_url);
                self
            }

            /// Set the organization ID for the builder.
            pub fn with_organization_id(mut self, organization_id: &str) -> Self {
                self.inner = self.inner.with_organization_id(organization_id);
                self
            }

            /// Build the model.
            pub fn build(self) -> $ty {
                $ty {
                    inner: self.inner.build(),
                }
            }
        }

        impl $ty {
            /// Creates a new builder
            pub fn builder() -> $tybuilder {
                $tybuilder::new()
            }
        }

        impl Default for $ty {
            fn default() -> Self {
                Self::builder().build()
            }
        }

        #[async_trait::async_trait]
        impl ModelBuilder for $tybuilder {
            type Model = $ty;

            async fn start_with_loading_handler(
                self,
                _: impl FnMut(ModelLoadingProgress) + Send + Sync + 'static,
            ) -> anyhow::Result<$ty> {
                Ok($ty {
                    inner: self.inner.build(),
                })
            }

            fn requires_download(&self) -> bool {
                false
            }
        }

        #[async_trait::async_trait]
        impl crate::model::Model for $ty {
            type TextStream = ChannelTextStream;
            type SyncModel = crate::SyncModelNotSupported;

            fn tokenizer(&self) -> Arc<Tokenizer> {
                panic!("OpenAI does not expose tokenization")
            }

            async fn stream_text_inner(
                &self,
                prompt: &str,
                generation_parameters: GenerationParameters,
            ) -> anyhow::Result<Self::TextStream> {
                self.inner
                    .stream_text_inner(prompt, generation_parameters)
                    .await
            }
        }
    };
}

openai_completion_model!(Gpt3_5, Gpt3_5Builder, "gpt-3.5-turbo-instruct");
// The rest of the openai models only support the chat API which currently isn't supported for remote models in kalosm
// openai_chat_model!(Gpt4, Gpt4Builder, "gpt-4");
// openai_chat_model!(Gpt4Turbo, Gpt4TurboBuilder, "gpt-4-turbo");
// openai_chat_model!(Gpt4O, Gpt4OBuilder, "gpt-4o");
// openai_chat_model!(Gpt4Mini, Gpt4MiniBuilder, "gpt-4o-mini");

/// An embedder that uses OpenAI's API for the Ada embedding model.
#[derive(Debug)]
pub struct AdaEmbedder {
    client: Client<async_openai::config::OpenAIConfig>,
}

/// A builder for the Ada embedder.
#[derive(Debug, Default)]
pub struct AdaEmbedderBuilder {
    config: async_openai::config::OpenAIConfig,
}

impl AdaEmbedderBuilder {
    /// Creates a new builder
    pub fn new() -> Self {
        Self {
            config: Default::default(),
        }
    }

    /// Sets the API key for the builder.
    pub fn with_api_key(mut self, api_key: &str) -> Self {
        self.config = self.config.with_api_key(api_key);
        self
    }

    /// Set the base URL of the API.
    pub fn with_base_url(mut self, base_url: &str) -> Self {
        self.config = self.config.with_api_base(base_url);
        self
    }

    /// Set the organization ID for the builder.
    pub fn with_organization_id(mut self, organization_id: &str) -> Self {
        self.config = self.config.with_org_id(organization_id);
        self
    }

    /// Build the model.
    pub fn build(self) -> AdaEmbedder {
        AdaEmbedder {
            client: Client::with_config(self.config),
        }
    }
}

impl AdaEmbedder {
    /// Creates a new builder
    pub fn builder() -> AdaEmbedderBuilder {
        AdaEmbedderBuilder::new()
    }
}

impl Default for AdaEmbedder {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl ModelBuilder for AdaEmbedderBuilder {
    type Model = AdaEmbedder;

    async fn start_with_loading_handler(
        self,
        _: impl FnMut(ModelLoadingProgress) + Send + Sync + 'static,
    ) -> anyhow::Result<AdaEmbedder> {
        Ok(self.build())
    }

    fn requires_download(&self) -> bool {
        false
    }
}

/// The embedding space for the Ada embedding model.
pub struct AdaEmbedding;

impl VectorSpace for AdaEmbedding {}

impl AdaEmbedder {
    /// The model ID for the Ada embedding model.
    pub const MODEL_ID: &'static str = "text-embedding-ada-002";
}

impl Embedder for AdaEmbedder {
    type VectorSpace = AdaEmbedding;

    fn embed_for(
        &self,
        input: crate::EmbeddingInput,
    ) -> BoxedFuture<'_, anyhow::Result<Embedding<Self::VectorSpace>>> {
        self.embed_string(input.text)
    }

    fn embed_vec_for(
        &self,
        inputs: Vec<crate::EmbeddingInput>,
    ) -> BoxedFuture<'_, anyhow::Result<Vec<Embedding<Self::VectorSpace>>>> {
        let inputs = inputs
            .into_iter()
            .map(|input| input.text)
            .collect::<Vec<_>>();
        self.embed_vec(inputs)
    }

    /// Embed a single string.
    fn embed_string(
        &self,
        input: String,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Embedding<AdaEmbedding>>> + Send + '_>> {
        Box::pin(async move {
            let request = CreateEmbeddingRequestArgs::default()
                .model(Self::MODEL_ID)
                .input([input])
                .build()?;
            let response = self.client.embeddings().create(request).await?;

            let embedding = Embedding::from(response.data[0].embedding.iter().copied());

            Ok(embedding)
        })
    }

    /// Embed a single string.
    fn embed_vec(
        &self,
        input: Vec<String>,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<Embedding<AdaEmbedding>>>> + Send + '_>>
    {
        Box::pin(async move {
            let request = CreateEmbeddingRequestArgs::default()
                .model(Self::MODEL_ID)
                .input(input)
                .build()?;
            let response = self.client.embeddings().create(request).await?;

            Ok(response
                .data
                .into_iter()
                .map(|data| Embedding::from(data.embedding.into_iter()))
                .collect())
        })
    }
}
