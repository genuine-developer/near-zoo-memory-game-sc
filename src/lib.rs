use near_contract_standards::non_fungible_token::core::NonFungibleTokenCore;
use near_contract_standards::non_fungible_token::metadata::{
    NFTContractMetadata, NonFungibleTokenMetadataProvider, TokenMetadata, NFT_METADATA_SPEC,
};
use near_contract_standards::non_fungible_token::{NonFungibleToken, Token, TokenId};
use near_sdk::borsh::{self, BorshDeserialize, BorshSerialize};
use near_sdk::collections::LookupMap;
use near_sdk::collections::*;
use near_sdk::json_types::ValidAccountId;
use near_sdk::serde::{Deserialize, Serialize};
use near_sdk::{
    env, ext_contract, near_bindgen, AccountId, Balance, BorshStorageKey, PanicOnDefault, Promise,
    PromiseOrValue,
};

use crate::userdata::*;
mod userdata;

const POINTS_PER_LEVEL: [u32; 11] = [100, 120, 140, 160, 180, 200, 220, 240, 260, 280, 300];

const MINT_NFT_FEE: Balance = 1_000_000_000_000_000_000_000_00;
const AUCTION_FEE: Balance = 1_000_000_000_000_000_000_000_000;
const DATA_IMAGE_SVG_NEAR_ICON: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 288 288'%3E%3Cg id='l' data-name='l'%3E%3Cpath d='M187.58,79.81l-30.1,44.69a3.2,3.2,0,0,0,4.75,4.2L191.86,103a1.2,1.2,0,0,1,2,.91v80.46a1.2,1.2,0,0,1-2.12.77L102.18,77.93A15.35,15.35,0,0,0,90.47,72.5H87.34A15.34,15.34,0,0,0,72,87.84V201.16A15.34,15.34,0,0,0,87.34,216.5h0a15.35,15.35,0,0,0,13.08-7.31l30.1-44.69a3.2,3.2,0,0,0-4.75-4.2L96.14,186a1.2,1.2,0,0,1-2-.91V104.61a1.2,1.2,0,0,1,2.12-.77l89.55,107.23a15.35,15.35,0,0,0,11.71,5.43h3.13A15.34,15.34,0,0,0,216,201.16V87.84A15.34,15.34,0,0,0,200.66,72.5h0A15.35,15.35,0,0,0,187.58,79.81Z'/%3E%3C/g%3E%3C/svg%3E";

#[derive(Debug, BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[serde(crate = "near_sdk::serde")]
pub struct Auction {
    owner: AccountId,
    auction_id: u128,
    auction_token: TokenId,
    start_price: Balance,
    start_time: u64,
    end_time: u64,
    current_price: Balance,
    winner: AccountId,
    is_near_claimed: bool,
    is_nft_claimed: bool,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Contract {
    owner: AccountId,
    tokens: NonFungibleToken,
    metadata: LazyOption<NFTContractMetadata>,
    total_auctions: u128,
    auction_by_id: UnorderedMap<u128, Auction>,
    auctions_by_owner: UnorderedMap<AccountId, Vector<u128>>,
    auctioned_tokens: UnorderedSet<TokenId>,
    current_token_number: u64,

    // for memory game
    records: LookupMap<String, UserInfo>,
}

#[derive(BorshSerialize, BorshStorageKey)]
enum StorageKey {
    NonFungibleToken,
    Metadata,
    TokenMetadata,
    Enumeration,
    Approval,

    InfoByUser,
}

#[ext_contract(ex_self)]
trait MyContract {
    fn external_mint(
        &mut self,
        token_id: TokenId,
        token_owner_id: ValidAccountId,
        token_metadata: Option<TokenMetadata>,
    );
}

near_contract_standards::impl_non_fungible_token_approval!(Contract, tokens);
near_contract_standards::impl_non_fungible_token_enumeration!(Contract, tokens);

#[near_bindgen]
impl Contract {
    // Init Contract
    #[init]
    pub fn new() -> Self {
        assert!(!env::state_exists(), "Already initialized");
        let metadata = NFTContractMetadata {
            spec: NFT_METADATA_SPEC.to_string(),
            name: "Zoo Memory NFT".to_string(),
            symbol: "ZMT".to_string(),
            icon: Some(DATA_IMAGE_SVG_NEAR_ICON.to_string()),
            base_uri: None,
            reference: None,
            reference_hash: None,
        };

        Self {
            owner: env::predecessor_account_id(), //msg.sender
            tokens: NonFungibleToken::new(
                StorageKey::NonFungibleToken,
                ValidAccountId::try_from(env::predecessor_account_id()).unwrap(),
                Some(StorageKey::TokenMetadata),
                Some(StorageKey::Enumeration),
                Some(StorageKey::Approval),
            ),
            metadata: LazyOption::new(StorageKey::Metadata, Some(&metadata)),

            total_auctions: 0,
            auction_by_id: UnorderedMap::new(b"auction_by_id".to_vec()), // auction => id
            auctions_by_owner: UnorderedMap::new(b"auctions_by_owner".to_vec()), // address => ids[]
            auctioned_tokens: UnorderedSet::new(b"is_token_auctioned".to_vec()), //
            current_token_number: 1,

            records: LookupMap::new(StorageKey::InfoByUser.try_to_vec().unwrap()),
        }
    }

    // for memory game --> start
    pub fn set_status(&mut self) -> UserInfo {
        let account_id = env::signer_account_id();
        let current_block_time = (near_sdk::env::block_timestamp() / 1000_000_000) as u64;
        let user_info: UserInfo;

        if let Some(user_data) = self.records.get(&account_id) {
            if current_block_time > user_data.last_updated_at + 24 * 60 * 60 {
                user_info = UserInfo {
                    level: 1,
                    temp_points: POINTS_PER_LEVEL[0],
                    last_updated_at: current_block_time,
                    ..user_data
                };
            } else {
                user_info = UserInfo {
                    level: user_data.level + 1,
                    temp_points: user_data.temp_points + POINTS_PER_LEVEL[user_data.level as usize],
                    last_updated_at: current_block_time,
                    ..user_data
                };
            }
        } else {
            user_info = UserInfo {
                level: 1,
                temp_points: POINTS_PER_LEVEL[0],
                perm_points: 0,
                last_updated_at: current_block_time,
            };
        }

        self.records.insert(&account_id, &user_info);

        user_info
    }

    pub fn get_status(&self, account_id: String) -> Option<UserInfo> {
        if let Some(user_info) = self.records.get(&account_id) {
            Some(user_info)
        } else {
            None
        }
    }
    // for memory game --> end

    // Mint TokenId
    #[payable]
    pub fn nft_mint(
        &mut self,
        token_id: TokenId,
        receiver_id: ValidAccountId,
        token_metadata: Option<TokenMetadata>,
    ) -> Token {
        assert_eq!(
            env::attached_deposit(),
            MINT_NFT_FEE,
            "Marketplace: mint fee must be greater than MINT_FEE"
        );

        // mint
        self.current_token_number += 1;
        self.tokens.mint(token_id, receiver_id, token_metadata)
    }

    // Transfer Token
    #[payable]
    pub fn nft_transfer(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
    ) {
        self.tokens
            .nft_transfer(receiver_id, token_id, approval_id, memo)
    }

    #[payable]
    pub fn nft_transfer_call(
        &mut self,
        receiver_id: ValidAccountId,
        token_id: TokenId,
        approval_id: Option<u64>,
        memo: Option<String>,
        msg: String,
    ) -> PromiseOrValue<bool> {
        self.tokens
            .nft_transfer_call(receiver_id, token_id, approval_id, memo, msg)
    }

    pub fn nft_token(self, token_id: TokenId) -> Option<Token> {
        self.tokens.nft_token(token_id)
    }

    // Create Auction
    #[payable]
    pub fn create_auction(
        &mut self,
        auction_token: TokenId,
        start_price: Balance,
        start_time: u64,
        end_time: u64,
    ) -> Auction {
        let owner_id = self.tokens.owner_by_id.get(&auction_token).unwrap();
        assert_eq!(
            owner_id,
            env::predecessor_account_id(),
            "You are not owner of nft"
        );
        assert_eq!(
            self.auctioned_tokens.contains(&auction_token),
            false,
            "Already auctioned"
        );
        // assert_eq!(
        //     env::attached_deposit(),
        //     AUCTION_FEE,
        //     "Maketplace: fee mus be greater than CREATE_AUCTION_FEE"
        // );

        // Token se duoc gui vao contract

        self.tokens.internal_transfer(
            &env::predecessor_account_id(),
            &env::current_account_id(), // dia chi contract
            &auction_token,
            None,
            None,
        );

        let mut auction_ids: Vector<u128>;

        if self
            .auctions_by_owner
            .get(&env::predecessor_account_id())
            .is_none()
        {
            auction_ids = Vector::new(b"auction_ids".to_vec());
        } else {
            auction_ids = self
                .auctions_by_owner
                .get(&env::predecessor_account_id())
                .unwrap();
        }

        auction_ids.push(&self.total_auctions);

        let auction = Auction {
            owner: owner_id,
            auction_id: self.total_auctions,
            auction_token: auction_token.clone(),
            start_price,
            start_time: start_time,
            end_time: end_time,
            current_price: start_price,
            winner: String::new(),
            is_near_claimed: false,
            is_nft_claimed: false,
        };

        self.auctions_by_owner
            .insert(&env::predecessor_account_id(), &auction_ids);
        self.auction_by_id.insert(&self.total_auctions, &auction);
        self.auctioned_tokens.insert(&auction_token);
        self.total_auctions += 1;

        auction
    }

    // Bid On Token
    #[payable]
    pub fn bid(&mut self, auction_id: u128) {
        let mut auction = self.auction_by_id.get(&auction_id).unwrap_or_else(|| {
            panic!("This auction does not exist");
        });

        assert_eq!(
            env::block_timestamp() > auction.start_time,
            true,
            "This auction has not started"
        );
        assert_eq!(
            env::block_timestamp() < auction.end_time,
            true,
            "This auction has already done"
        );

        assert_eq!(
            env::attached_deposit() > auction.current_price,
            true,
            "Price must be greater than current winner's price"
        );

        // Neu chua co winner thi set winner, neu ton tai thi back lai tien
        if !(auction.winner == String::new()) {
            let old_owner = Promise::new(auction.winner);
            old_owner.transfer(auction.current_price);
        }

        auction.winner = env::predecessor_account_id();
        auction.current_price = env::attached_deposit();
        self.auction_by_id.insert(&auction_id, &auction);
    }

    // claim_nft cho winner
    #[payable]
    pub fn claim_nft(&mut self, auction_id: u128) {
        let mut auction = self.auction_by_id.get(&auction_id).unwrap_or_else(|| {
            panic!("This auction does not exist");
        });

        assert_eq!(
            env::block_timestamp() > auction.end_time,
            true,
            "This auction is not over yet"
        );
        assert_eq!(
            env::predecessor_account_id(),
            auction.winner,
            "You are not winner"
        );
        assert_eq!(
            auction.clone().is_near_claimed,
            false,
            "You has already claimed NFT"
        );

        self.tokens.internal_transfer_unguarded(
            &auction.auction_token,
            &env::current_account_id(),
            &auction.winner,
        );

        auction.is_near_claimed = true;
        self.auctioned_tokens.remove(&auction.auction_token);
        self.auction_by_id.insert(&auction_id, &auction);
    }

    // claim_near cho auction owner
    #[payable]
    pub fn claim_near(&mut self, auction_id: u128) {
        let mut auction = self.auction_by_id.get(&auction_id).unwrap_or_else(|| {
            panic!("This auction does not exist");
        });

        assert_eq!(
            env::predecessor_account_id(),
            auction.owner,
            "You are not operator of this auction"
        );
        assert_eq!(
            env::block_timestamp() > auction.end_time,
            true,
            "This auction is not over yet"
        );

        assert_eq!(auction.is_near_claimed, false, "You has already claimed");

        Promise::new(auction.clone().owner).transfer(auction.current_price - AUCTION_FEE);
        auction.is_near_claimed = true;

        self.auction_by_id.insert(&auction_id, &auction);
    }

    // claimnft cho owner neu k co ai bid
    #[payable]
    pub fn claim_back_nft(&mut self, auction_id: u128) {
        let mut auction = self.auction_by_id.get(&auction_id).unwrap_or_else(|| {
            panic!("This auction does not exist");
        });

        assert_eq!(
            env::predecessor_account_id(),
            auction.owner,
            "You are not operator of this auction"
        );
        assert_eq!(
            env::block_timestamp() > auction.end_time,
            true,
            "This auction is not over yet"
        );
        assert_eq!(auction.winner, String::new(), "Have no bidder");

        self.tokens.internal_transfer_unguarded(
            &auction.auction_token,
            &env::current_account_id(),
            &auction.owner,
        );

        auction.is_nft_claimed = true;
        self.auctioned_tokens.remove(&auction.auction_token);
        self.auction_by_id.insert(&auction_id, &auction);
    }

    pub fn get_auction(&mut self, auction_id: u128) -> Auction {
        self.auction_by_id.get(&auction_id).unwrap()
    }

    pub fn get_current_token_number(&self) -> u64 {
        self.current_token_number
    }
}

#[near_bindgen]
impl NonFungibleTokenMetadataProvider for Contract {
    fn nft_metadata(&self) -> NFTContractMetadata {
        self.metadata.get().unwrap()
    }
}
