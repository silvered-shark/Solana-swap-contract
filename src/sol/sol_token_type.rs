use solana_client::{client_error::reqwest::Error, nonblocking::pubsub_client::PubsubClient};
use solana_sdk::pubkey::Pubkey;
use spl_token_2022::{extension::StateWithExtensionsOwned, state::Mint};

use super::{SolError, SolMut};

pub enum SolTokenType {
    SplToken,
    SplToken2022,
}

impl SolTokenType {
    pub async fn get_decimals(
        &self,
        mint: &Pubkey,
    ) -> Result<u8, SolError> {
        let client = SolMut::get_solana_client();
        match self {
            SolTokenType::SplToken => todo!(),
            SolTokenType::SplToken2022 => {
                let account = client.get_account_data(mint).await.map_err(|_| SolError::OwnerNotToken)?;
                let mint = StateWithExtensionsOwned::<Mint>::unpack(account).map_err(|_| SolError::OwnerNotToken)?;
                Ok(mint.base.decimals)
            }
        }
    }
    pub fn id(&self) -> Pubkey {
        match self {
            SolTokenType::SplToken => spl_token::ID,
            SolTokenType::SplToken2022 => spl_token_2022::ID,
        }
    }
    pub async fn detect_token_program(mint: &Pubkey) -> Result<Self, SolError> {
        let client = SolMut::get_solana_client();
        let token = client.get_account(mint).await.map_err(|_| SolError::OwnerNotToken)?;
        Ok(match token.owner {
            spl_token::ID => Self::SplToken,
            spl_token_2022::ID => Self::SplToken2022,
            _ => return Err(SolError::OwnerNotToken),
        })
    }
}
