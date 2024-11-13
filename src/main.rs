use blazing_agi::{command::{verbose::Verbose, AGIResponse, GetFullVariable}, connection::Connection, handler::AGIHandler, router::Router, serve, AGIError, AGIRequest};
use blazing_agi_macros::layer_before;
use rand::Rng;
use sha1::{Digest, Sha1};
use tokio::net::TcpListener;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*, EnvFilter};

mod config;
use config::{CmiConfig, Config, RoomMapping};



#[derive(Debug, Clone)]
enum SHA1DigestError {
    DecodeError,
    WrongDigest,
}
impl core::fmt::Display for SHA1DigestError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::DecodeError => {
                write!(f, "The returned digest was not decodable as u8")
            }
            Self::WrongDigest => {
                write!(f, "The returned digest is false")
            }
        }
    }
}
impl std::error::Error for SHA1DigestError {}

/// Create a 20-byte Nonce with 8 bytes of Randomness, encoded as a hex string
fn create_nonce() -> String {
    let mut raw_bytes = [0_u8; 20];
    let now_in_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("Should be after the epoch");
    // 8 bytes against reuse
    raw_bytes[0..=7].clone_from_slice(&now_in_secs.as_secs().to_le_bytes());
    // 4 bytes against reuse
    raw_bytes[8..=11].clone_from_slice(&now_in_secs.subsec_millis().to_le_bytes());
    // 8 bytes against predictability
    rand::rngs::ThreadRng::default().fill(&mut raw_bytes[12..=19]);
    return hex::encode(raw_bytes);
}

#[derive(Clone, Debug)]
struct SHA1DigestOverAGI {
    secret: String,
}
impl SHA1DigestOverAGI {
    pub fn new<S: AsRef<str>>(secret: S) -> Self {
        Self {
            secret: secret.as_ref().to_string(),
        }
    }
}
#[async_trait::async_trait]
impl AGIHandler for SHA1DigestOverAGI {
    // Note: this handler does not care about the request.
    // It simply ignores it and does the AGI digest.
    // This handler effectively works as a layer later)
    //
    // In asterisk, you have to set the same secret as follows:
    // same => n,Set(BLAZING_AGI_DIGEST_SECRET=top_secret)
    async fn handle(&self, connection: &mut Connection, _: &AGIRequest) -> Result<(), AGIError> {
        let nonce = create_nonce();
        let mut hasher = Sha1::new();
        hasher.update(self.secret.as_bytes());
        hasher.update(":".as_bytes());
        hasher.update(&nonce.as_bytes());
        let expected_digest: [u8; 20] = hasher.finalize().into();
        let digest_response = connection
            .send_command(GetFullVariable::new(format!(
                "${{SHA1(${{BLAZING_AGI_DIGEST_SECRET}}:{})}}",
                nonce
            )))
            .await?;
        match digest_response {
            AGIResponse::Ok(inner_response) => {
                if let Some(digest_as_str) = inner_response.value {
                    if expected_digest
                        != *hex::decode(digest_as_str).map_err(|_| {
                            AGIError::InnerError(Box::new(SHA1DigestError::DecodeError))
                        })?
                    {
                        connection
                            .send_command(Verbose::new(
                                "Unauthenticated: Wrong Digest.".to_string(),
                            ))
                            .await?;
                        Err(AGIError::InnerError(Box::new(SHA1DigestError::WrongDigest)))
                    } else {
                        Ok(())
                    }
                } else {
                    Err(AGIError::ClientSideError(
                        "Expected BLAZING_AGI_DIGEST_SECRET to be set, but it is not".to_string(),
                    ))
                }
            }
            m => {
                return Err(AGIError::Not200(m.into()));
            }
        }
    }
}


#[derive(Debug)]
struct OpenDoorHandler {
    config: CmiConfig,
}
impl OpenDoorHandler {
    fn get_cmi_for_room<S: AsRef<str>>(&self, room_name: S) -> Option<&RoomMapping> {
        self.config.get_cmi_for_room(room_name.as_ref())
    }
}
#[async_trait::async_trait]
impl AGIHandler for OpenDoorHandler {
    async fn handle(&self, connection: &mut Connection, request: &AGIRequest) -> Result<(), AGIError> {
        // make sure the room is known
        let room = request.captures.get("room").ok_or(AGIError::ClientSideError("Got no captured room".to_owned()))?;
        // get the cmi connection used for this room
        connection.send_command(Verbose::new(format!("The room {room} is not known."))).await?;
        let cmi_config = self.get_cmi_for_room(room).ok_or(AGIError::ClientSideError("Room is not known.".to_owned()))?;
        // send ON to that CMI
        cmi_config.open_door().await.map_err(|x| AGIError::ClientSideError(x.to_string()))?;
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // create the logger
    let my_crate_filter = EnvFilter::new("ta_agi_doorbell");
    let subscriber = tracing_subscriber::registry().with(my_crate_filter).with(
        tracing_subscriber::fmt::layer()
            .compact()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_line_number(true)
            .with_filter(LevelFilter::TRACE),
    );
    tracing::subscriber::set_global_default(subscriber).expect("static tracing config");

    // setup config
    let config = Config::create()?;
    let digest_secret = config.agi_digest_secret();
    let agi_listen_string = config.agi_listen_string();

    // Create the router from the handlers you have defined
    let router = Router::new()
        .route("/open_door/:room", OpenDoorHandler { config: config.cmi })
        .layer(layer_before!(SHA1DigestOverAGI::new(digest_secret)));

    let listener = TcpListener::bind(agi_listen_string).await?;
    // Start serving the Router
    serve::serve(listener, router).await?;
    Ok(())
}
