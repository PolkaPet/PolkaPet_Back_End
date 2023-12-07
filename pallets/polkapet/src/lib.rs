#![cfg_attr(not(feature = "std"), no_std)]

/// Edit this file to define custom logic or remove it if it is not needed.
/// Learn more about FRAME and the core library of Substrate FRAME pallets:
/// <https://docs.substrate.io/reference/frame-pallets/>
pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;
pub mod weights;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{DispatchResult, DispatchResultWithPostInfo, GetDispatchInfo, Vec},
		ensure,
		pallet_prelude::{ValueQuery, *},
		sp_runtime::{
			traits::{AccountIdConversion, SaturatedConversion},
			ArithmeticError,
		},
		traits::{Currency, ExistenceRequirement, Randomness, UnfilteredDispatchable},
		Blake2_128Concat, Hashable, PalletId,
	};
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor, *};
	use scale_info::{prelude::string::String, TypeInfo};
	use sp_runtime::{FixedPointNumber, FixedU128};

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	// Hooks for update onchain value
	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(block_number: BlockNumberFor<T>) {
			let current_block_number = block_number.saturated_into::<u32>();
			if current_block_number % 5 == 0 {
				<OvalBoostNumber<T>>::put(Self::random_oval_boost_value());
			}

			if current_block_number % 50 == 0 {
				Self::kill_random_pet();
			}

			if current_block_number % 1 == 0 {
				let _ = Self::operate_running_game(current_block_number);
			}
		}
	}

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	type PetId = u64;
	type MinigameId = u64;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_insecure_randomness_collective_flip::Config
	{
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The Currency handler for the Polkapets pallet.
		type Currency: Currency<Self::AccountId>;

		type MyRandomness: Randomness<Self::Hash, BlockNumberFor<Self>>;

		/// A sudo-able call.
		type RuntimeCall: Parameter
			+ UnfilteredDispatchable<RuntimeOrigin = Self::RuntimeOrigin>
			+ GetDispatchInfo;

		/// Type representing the weight of this pallet
		type WeightInfo: WeightInfo;

		/// The maximum amount of kitties a single account can own.
		#[pallet::constant]
		type MaxPolkapetOwned: Get<u32>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new polkapet was successfully created.
		Created { polkapet: PetId, owner: T::AccountId },
		/// The price of a polkapet was successfully set.
		PriceSet { polkapet: PetId, price: Option<BalanceOf<T>> },
		/// A polkapet was successfully transferred.
		Transferred { from: T::AccountId, to: T::AccountId, polkapet: PetId },
		/// A polkapet was successfully sold.
		Sold { seller: T::AccountId, buyer: T::AccountId, polkapet: PetId, price: BalanceOf<T> },
		/// a polkapet was empowered
		Empowered { polkapet: PetId, power: u32 },
		/// update pet death status
		UpdatePetDeathStatus { polkapet: PetId },
		/// a pet was respawned
		Respawn { polkapet: PetId },
		/// a game with reward was created
		GameCreated { id: u64, reward: BalanceOf<T> },
	}

	// Set Gender type in polkapet struct
	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum Gender {
		Male,
		Female,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum DotLogoOvalPosition {
		Up,
		RightUp,
		LeftUp,
		RightDown,
		LeftDown,
		Down,
	}

	#[derive(Clone, Encode, Decode, PartialEq, Copy, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub enum GameStatus {
		Prepare,
		OnGoing,
		Finish,
	}

	// Set DotLogoOval type for polkapet struct
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, Default)]
	#[scale_info(skip_type_params(T))]
	pub struct DotLogoOvalPositionBoost {
		pub up: u8,
		pub right_up: u8,
		pub left_up: u8,
		pub right_down: u8,
		pub left_down: u8,
		pub down: u8,
	}

	// Struct for holding polkapet information
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, Copy)]
	#[scale_info(skip_type_params(T))]
	pub struct Polkapet<T: Config> {
		pub dna: [u8; 16],
		pub price: Option<BalanceOf<T>>,
		pub gender: Gender,
		pub owner: T::AccountId,
		pub oval_position: DotLogoOvalPosition,
		pub power: u32,
		pub death: bool,
		pub respawn: u32,
		pub pet_id: PetId,
		pub is_join_minigame: bool,
	}

	// Struct for holding minigame information
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct Minigame<T: Config> {
		pub game_id: MinigameId,
		pub owner: T::AccountId,
		pub description: Vec<u8>,
		pub reward: Option<BalanceOf<T>>,
		pub max_player: Option<u32>,
		pub block_duration: u32,
		pub finish_block: u32,
		pub status: GameStatus,
	}

	// Struct for holding minigame information
	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo)]
	#[scale_info(skip_type_params(T))]
	pub struct MinigameResult {
		pub pet_id: PetId,
		pub point: u32,
	}

	// #[pallet::storage]
	// #[pallet::getter(fn max_minigame_duration)]
	// pub type MaxMinigameDuration<T> = StorageValue<_, u32>;

	#[pallet::storage]
	#[pallet::getter(fn last_pet_id)]
	/// Keeps track of the last id of Polkapets.
	pub type LastPetId<T: Config> = StorageValue<_, PetId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_pet_by_id)]
	pub type PolkapetsById<T: Config> = StorageMap<_, Blake2_128Concat, PetId, Polkapet<T>>;

	#[pallet::storage]
	pub type PolkapetsOwned<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<PetId, T::MaxPolkapetOwned>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub type OvalBoostNumber<T: Config> = StorageValue<_, DotLogoOvalPositionBoost, ValueQuery>;

	/// Keeps track of the last id of Minigame.
	#[pallet::storage]
	#[pallet::getter(fn last_minigame_id)]
	pub type LastMinigameId<T: Config> = StorageValue<_, MinigameId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn get_minigame_by_id)]
	pub type MinigameById<T: Config> = StorageMap<_, Blake2_128Concat, MinigameId, Minigame<T>>;

	#[pallet::storage]
	#[pallet::getter(fn get_minigame_players)]
	pub type MinigamePlayers<T: Config> =
		StorageMap<_, Blake2_128Concat, MinigameId, Vec<Polkapet<T>>>;

	/// Mapping minigame owned account and minigame IDs
	#[pallet::storage]
	pub type MinigameOwned<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MinigameId>, ValueQuery>;

	/// Keeps track of the running id of Minigames.
	#[pallet::storage]
	#[pallet::getter(fn running_minigame_id)]
	pub type RunningMinigameIds<T: Config> = StorageValue<_, Vec<MinigameId>, ValueQuery>;

	/// Keeps track of the Minigame results.
	#[pallet::storage]
	#[pallet::getter(fn finished_minigame_results)]
	pub type MinigameResultsByMinigameId<T: Config> =
		StorageMap<_, Blake2_128Concat, MinigameId, Vec<MinigameResult>>;

	// Errors.
	#[pallet::error]
	pub enum Error<T> {
		/// An account may only own `MaxPolkapetsOwned` kitties.
		TooManyOwned,
		/// Trying to transfer or buy a polkapet from oneself.
		TransferToSelf,
		/// This polkapet already exists!
		DuplicatePolkapet,
		/// This polkapet does not exist!
		NoPolkapet,
		/// You are not the owner of this polkapet.
		NotOwner,
		/// This polkapet is not for sale.
		NotForSale,
		/// Ensures that the buying price is greater than the asking price.
		BidPriceTooLow,
		/// You need to have two cats with different gender to breed.
		CantBreed,
		/// Handles arithemtic overflow when incrementing the pet Id.
		PetIdOverflow,
		/// Debug error
		OtherError,
		/// Handles arithemtic overflow when incrementing the MinigameId
		MinigameIdOverflow,
		/// Handles error when creating minigame with invalid block duration
		BlockDurationInvalid,
		/// Handles error when get minigame with invalid id
		NoMinigame,
		/// Handles error when join minigame after it start
		GameStartedOrEnded,
		/// Handles error when pet already joined minigame
		AlreadyJoinGame,
		/// Handles error when start a minigame with no player
		NoPlayer,
		/// Excess number of players beyond the maximum limit
		ExceedingMaxPlayers,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn create_polkapet(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let pet_id = Self::get_new_pet_id_and_increase()?;

			// Generate unique DNA and Gender
			let (polkapet_gen_dna, gender) = Self::gen_dna();
			let oval_position = Self::gen_oval_position();

			let _ = Self::mint(&sender, polkapet_gen_dna, gender, oval_position, pet_id)?;

			frame_support::log::info!("A polkapet is born with ID: {:?}.", pet_id);

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(10_000)]
		pub fn set_price(
			origin: OriginFor<T>,
			pet_id: PetId,
			new_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			// Ensure the polkapet exists and is called by the polkapet owner
			let mut polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			ensure!(polkapet.owner == sender, Error::<T>::NotOwner);

			// Set the price in storage
			polkapet.price = new_price;

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(&polkapet.clone().pet_id, polkapet);

			Self::deposit_event(Event::PriceSet { polkapet: pet_id, price: new_price });

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(10_000)]
		pub fn transfer(origin: OriginFor<T>, to: T::AccountId, pet_id: PetId) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let from = ensure_signed(origin)?;
			let polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			ensure!(polkapet.owner == from, Error::<T>::NotOwner);
			Self::do_transfer(pet_id, to, None)?;
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn buy_polkapet(
			origin: OriginFor<T>,
			pet_id: PetId,
			limit_price: BalanceOf<T>,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let buyer = ensure_signed(origin)?;
			// Transfer the polkapet from seller to buyer as a sale
			Self::do_transfer(pet_id, buyer, Some(limit_price))?;

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn breed_polkapet(
			origin: OriginFor<T>,
			parent_1: PetId,
			parent_2: PetId,
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			let maybe_mom = PolkapetsById::<T>::get(&parent_1).ok_or(Error::<T>::NoPolkapet)?;
			let maybe_dad = PolkapetsById::<T>::get(&parent_2).ok_or(Error::<T>::NoPolkapet)?;

			// Check both parents are owned by the caller of this function
			ensure!(maybe_mom.owner == sender, Error::<T>::NotOwner);
			ensure!(maybe_dad.owner == sender, Error::<T>::NotOwner);

			// Parents must be of opposite genders
			ensure!(maybe_mom.gender != maybe_dad.gender, Error::<T>::CantBreed);

			// Create new DNA from these parents
			let (new_dna, new_gender) = Self::breed_dna(&maybe_mom.dna, &maybe_dad.dna);
			let oval_position = Self::gen_oval_position();

			let pet_id = Self::get_new_pet_id_and_increase()?;

			// Mint new polkapet
			Self::mint(&sender, new_dna, new_gender, oval_position, pet_id)?;
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(10_000)]
		pub fn empower(origin: OriginFor<T>, pet_id: PetId) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			let mut polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			let oval_boost_number = OvalBoostNumber::<T>::get();
			let empowed_boost = match polkapet.oval_position {
				DotLogoOvalPosition::Up => oval_boost_number.up,
				DotLogoOvalPosition::RightUp => oval_boost_number.right_up,
				DotLogoOvalPosition::LeftUp => oval_boost_number.left_up,
				DotLogoOvalPosition::RightDown => oval_boost_number.right_down,
				DotLogoOvalPosition::LeftDown => oval_boost_number.left_down,
				DotLogoOvalPosition::Down => oval_boost_number.down,
			};

			let new_power = polkapet
				.power
				.checked_add(empowed_boost.into())
				.ok_or(ArithmeticError::Overflow)?;
			polkapet.power = new_power;

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(polkapet.pet_id, polkapet.clone());

			// Emit an event.
			Self::deposit_event(Event::Empowered { polkapet: polkapet.pet_id, power: new_power });
			// Return a successful DispatchResultWithPostInfo
			Ok(Pays::No.into())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(10_000)]
		pub fn respawn(origin: OriginFor<T>, pet_id: PetId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let mut polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			ensure!(polkapet.owner == who, Error::<T>::NotOwner);

			polkapet.respawn = polkapet.respawn.checked_add(1).ok_or(ArithmeticError::Overflow)?;
			// Minus 10% power
			polkapet.power = (polkapet.power as f64 * 0.9) as u32;
			polkapet.death = false;

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(polkapet.pet_id, polkapet.clone());

			// Emit an event.
			Self::deposit_event(Event::Respawn { polkapet: pet_id });
			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		// Minigame extensis

		#[pallet::call_index(7)]
		#[pallet::weight(10_000)]
		pub fn create_minigame(
			origin: OriginFor<T>,
			description: Vec<u8>,
			reward: Option<BalanceOf<T>>,
			max_player: Option<u32>,
			block_duration: u32,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			//Block duration must be between 0 and 100
			ensure!(block_duration > 0 && block_duration <= 100, Error::<T>::BlockDurationInvalid);
			let new_game_id = Self::get_new_minigame_id_and_increase()?;
			let new_game = Minigame::<T> {
				game_id: new_game_id,
				owner: sender.clone(),
				description,
				reward,
				max_player,
				block_duration,
				finish_block: 0,
				status: GameStatus::Prepare,
			};

			// todo!("Minus token from sender");
			if let Some(value) = reward {
				T::Currency::transfer(
					&sender,
					&Self::account_id(),
					value,
					ExistenceRequirement::KeepAlive,
				)?;
				// Deposit sold event
				Self::deposit_event(Event::GameCreated { id: new_game_id, reward: value });
			} else {
				frame_support::log::info!("No Reward Game created.");
			}

			// Create new minigame
			// Append minigame to MinigameOwner
			MinigameOwned::<T>::append(&sender, new_game_id);

			// Write new minigame to storage
			MinigameById::<T>::insert(new_game_id, new_game);

			frame_support::log::info!("A newgame was created with ID: {:?}.", new_game_id);

			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(10_000)]
		pub fn join_minigame(
			origin: OriginFor<T>,
			minigame_id: MinigameId,
			pet_id: PetId,
		) -> DispatchResultWithPostInfo {
			let _who = ensure_signed(origin)?;

			let minigame = MinigameById::<T>::get(minigame_id).ok_or(Error::<T>::NoMinigame)?;
			ensure!(minigame.status == GameStatus::Prepare, Error::<T>::GameStartedOrEnded);

			let mut polkapet = PolkapetsById::<T>::get(pet_id).ok_or(Error::<T>::NoPolkapet)?;

			ensure!(!polkapet.is_join_minigame, Error::<T>::AlreadyJoinGame);
			polkapet.is_join_minigame = true;

			let mut minigame_players = MinigamePlayers::<T>::get(minigame_id).unwrap_or_default();
			ensure!(!minigame_players.contains(&polkapet), Error::<T>::AlreadyJoinGame);

			// Check minigame max players
			if let Some(max_player) = minigame.max_player {
				ensure!(
					max_player > minigame_players.len() as u32,
					Error::<T>::ExceedingMaxPlayers
				);
			}

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(polkapet.pet_id, polkapet.clone());

			minigame_players.push(polkapet);
			MinigamePlayers::<T>::set(minigame_id, Some(minigame_players));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(10_000)]
		pub fn start_minigame(
			origin: OriginFor<T>,
			minigame_id: MinigameId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let mut minigame = MinigameById::<T>::get(minigame_id).ok_or(Error::<T>::NoMinigame)?;
			ensure!(minigame.status == GameStatus::Prepare, Error::<T>::GameStartedOrEnded);
			ensure!(minigame.owner == who, Error::<T>::NotOwner);

			// update minigame infors
			let current_blocknumber =
				frame_system::Pallet::<T>::block_number().saturated_into::<u32>();
			minigame.status = GameStatus::OnGoing;
			minigame.finish_block = current_blocknumber
				.checked_add(minigame.block_duration)
				.ok_or(ArithmeticError::Overflow)?;
			MinigameById::<T>::set(minigame_id, Some(minigame));

			// add minigame to running minigame map
			let mut running_game_ids = RunningMinigameIds::<T>::get();
			running_game_ids.push(minigame_id);
			RunningMinigameIds::<T>::set(running_game_ids);

			// Init minigame results
			let players = MinigamePlayers::<T>::get(minigame_id).unwrap_or_default();

			ensure!(players.len() > 0, Error::<T>::NoPlayer);

			let mut minigame_results =
				MinigameResultsByMinigameId::<T>::get(minigame_id).unwrap_or_default();
			for player in players.iter() {
				let result = MinigameResult { pet_id: player.pet_id, point: 0 };
				minigame_results.push(result);
			}
			MinigameResultsByMinigameId::<T>::set(minigame_id, Some(minigame_results));

			Ok(Pays::No.into())
		}

		#[pallet::call_index(10)]
		#[pallet::weight(10_000)]
		pub fn create_polkapet_bulk(origin: OriginFor<T>, q: u8) -> DispatchResult {
			for _ in 1..q {
				let _ = Self::create_polkapet(origin.clone());
			}

			// Return a successful DispatchResultWithPostInfo
			Ok(())
		}

		#[pallet::call_index(11)]
		#[pallet::weight(10_000)]
		pub fn remove_price(origin: OriginFor<T>, pet_id: PetId) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			// Ensure the polkapet exists and is called by the polkapet owner
			let mut polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			ensure!(polkapet.owner == sender, Error::<T>::NotOwner);

			// Set the price in storage
			polkapet.price = None;

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(&polkapet.clone().pet_id, polkapet);
			Self::deposit_event(Event::PriceSet { polkapet: pet_id, price: None });

			Ok(())
		}
	}

	// Helper function for Polkapet struct
	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		fn gen_dna() -> ([u8; 16], Gender) {
			let my_string = String::from("Gender").as_bytes().to_vec();
			let hash = Self::generate_random_hash(Some(my_string), None);

			// Generate Gender
			if hash[0] % 2 == 0 {
				// Males are identified by having a even leading byte
				(hash, Gender::Male)
			} else {
				// Females are identified by having a odd leading byte
				(hash, Gender::Female)
			}
		}

		fn gen_oval_position() -> DotLogoOvalPosition {
			let my_string = String::from("DotLogoOvalPosition").as_bytes().to_vec();
			let hash = Self::generate_random_hash(Some(my_string), None);

			// Generate DotLogoOvalPosition
			match hash[0] % 6 {
				0 => DotLogoOvalPosition::Up,
				1 => DotLogoOvalPosition::RightUp,
				2 => DotLogoOvalPosition::LeftUp,
				3 => DotLogoOvalPosition::RightDown,
				4 => DotLogoOvalPosition::LeftDown,
				_ => DotLogoOvalPosition::Down,
			}
		}

		pub fn mint(
			owner: &T::AccountId,
			dna: [u8; 16],
			gender: Gender,
			oval_position: DotLogoOvalPosition,
			pet_id: PetId,
		) -> DispatchResult {
			let polkapet = Polkapet::<T> {
				dna,
				price: None,
				gender,
				owner: owner.clone(),
				oval_position: oval_position.clone(),
				power: 0,
				death: false,
				respawn: 0,
				pet_id,
				is_join_minigame: false,
			};

			ensure!(!PolkapetsById::<T>::contains_key(&pet_id), Error::<T>::DuplicatePolkapet);

			// Append polkapet to PolkapetsOwner
			PolkapetsOwned::<T>::try_append(&owner, pet_id)
				.map_err(|_| Error::<T>::TooManyOwned)?;

			// Write new polkapet to storage
			PolkapetsById::<T>::insert(pet_id, polkapet);

			// Deposit our "Created" event.
			Self::deposit_event(Event::Created { polkapet: pet_id, owner: owner.clone() });

			// Returns the DNA of the new polkapet if this succeeds
			Ok(())
		}

		pub fn do_transfer(
			pet_id: PetId,
			to: T::AccountId,
			maybe_limit_price: Option<BalanceOf<T>>,
		) -> DispatchResult {
			let mut polkapet = PolkapetsById::<T>::get(&pet_id).ok_or(Error::<T>::NoPolkapet)?;
			let from = polkapet.owner;

			ensure!(from != to, Error::<T>::TransferToSelf);
			let mut from_owned = PolkapetsOwned::<T>::get(&from);

			// Remove polkapet from list of owned kitties.
			if let Some(ind) = from_owned.iter().position(|&id| id == pet_id) {
				from_owned.swap_remove(ind);
			} else {
				return Err(Error::<T>::NoPolkapet.into())
			}

			// Add polkapet to the list of owned kitties.
			let mut to_owned = PolkapetsOwned::<T>::get(&to);
			to_owned.try_push(pet_id).map_err(|_| Error::<T>::TooManyOwned)?;

			// Mutating state here via a balance transfer, so nothing is allowed to fail after this.
			// The buyer will always be charged the actual price. The limit_price parameter is just
			// a protection so the seller isn't able to front-run the transaction.
			if let Some(limit_price) = maybe_limit_price {
				if let Some(price) = polkapet.price {
					ensure!(limit_price >= price, Error::<T>::BidPriceTooLow);
					// Transfer the amount from buyer to seller
					T::Currency::transfer(&to, &from, price, ExistenceRequirement::KeepAlive)?;
					// Deposit sold event
					Self::deposit_event(Event::Sold {
						seller: from.clone(),
						buyer: to.clone(),
						polkapet: pet_id,
						price,
					});
				} else {
					// Polkapet price is set to `None` and is not for sale
					return Err(Error::<T>::NotForSale.into())
				}
			}
			// Transfer succeeded, update the polkapet owner and reset the price to `None`.
			polkapet.owner = to.clone();
			polkapet.price = None;

			// Write updates to storage
			PolkapetsById::<T>::insert(polkapet.pet_id, polkapet);

			PolkapetsOwned::<T>::insert(&to, to_owned);
			PolkapetsOwned::<T>::insert(&from, from_owned);

			Self::deposit_event(Event::Transferred { from, to, polkapet: pet_id });

			Ok(())
		}

		pub fn breed_dna(parent1: &[u8; 16], parent2: &[u8; 16]) -> ([u8; 16], Gender) {
			let (mut new_dna, new_gender) = Self::gen_dna();

			for i in 0..new_dna.len() {
				new_dna[i] = Self::mutate_dna_fragment(parent1[i], parent2[i], new_dna[i])
			}

			(new_dna, new_gender)
		}

		fn mutate_dna_fragment(dna_fragment1: u8, dna_fragment2: u8, random_value: u8) -> u8 {
			if random_value % 2 == 0 {
				// either return `dna_fragment1` if its an even value
				dna_fragment1
			} else {
				// or return `dna_fragment2` if its an odd value
				dna_fragment2
			}
		}

		fn random_oval_boost_value() -> DotLogoOvalPositionBoost {
			let my_string = String::from("DotLogoOvalPositionBoost").as_bytes().to_vec();
			let hash = Self::generate_random_hash(Some(my_string), None);

			let new_oval_boost = DotLogoOvalPositionBoost {
				up: hash[0] % 6 + 1,
				right_up: hash[1] % 6 + 1,
				left_up: hash[2] % 6 + 1,
				right_down: hash[3] % 6 + 1,
				left_down: hash[4] % 6 + 1,
				down: hash[5] % 6 + 1,
			};
			return new_oval_boost
		}

		fn kill_random_pet() {
			// get number of current pet
			let count = Self::last_pet_id();

			// percent of random kill x/10000
			let percent = 100_u64;

			// number of kill
			let death_number = count.checked_mul(percent).unwrap() / 10000;
			// let death_number = 2_u64;
			frame_support::log::info!("=======death_number {:?}.", death_number);

			let range = 0..death_number;
			range.for_each(|i| {
				let ran_pet_id = Self::generate_random_death_number(i).unwrap();
				frame_support::log::info!("=======Random pet id: {:?}.", ran_pet_id);
				if ran_pet_id > 0 && ran_pet_id <= count {
					Self::update_pet_death_status(ran_pet_id, true).unwrap();
					Self::update_pet_power(ran_pet_id, 0).unwrap();
					frame_support::log::info!("A polkapet was killed with ID: {:?}.", ran_pet_id);
					Self::deposit_event(Event::UpdatePetDeathStatus { polkapet: ran_pet_id });
				}
			});
		}

		/// Split the hash part out of the input.
		fn generate_random_death_number(i: u64) -> Result<PetId, DispatchError> {
			let my_string = String::from("randomDeathNumber").as_bytes().to_vec();
			let hash = Self::generate_random_hash(Some(my_string), Some(i));

			let mut result = 0_u64;

			let count = Self::last_pet_id();
			let count_in_vec_reverse = Self::generate_vec_reverse_digits_from_num(count)?;

			for (idx, ch) in count_in_vec_reverse.iter().enumerate() {
				let pow = 10_u64.pow(idx as u32);
				let mut ran_num: u64;

				if idx == (count_in_vec_reverse.len() - 1) {
					ran_num =
						PetId::from(hash[idx]) % ch.checked_add(1).ok_or(Error::<T>::OtherError)?;
				} else {
					ran_num = PetId::from(hash[idx]) % 10;
				}

				ran_num = ran_num.checked_mul(pow).ok_or(Error::<T>::OtherError)?;
				result = result + ran_num;
			}

			Ok(result)
		}

		// function to return the id of a new pet and to increase 1
		fn get_new_pet_id_and_increase() -> Result<PetId, DispatchError> {
			let current_last_id = &mut Self::last_pet_id();
			let new_last_id = current_last_id.checked_add(1).ok_or(Error::<T>::PetIdOverflow)?;

			LastPetId::<T>::put(new_last_id);
			Ok(new_last_id)
		}

		// function to return the id of a new pet and to increase 1
		fn update_pet_death_status(pet_id: PetId, status: bool) -> Result<(), DispatchError> {
			let mut new_pet = PolkapetsById::<T>::get(pet_id).ok_or(Error::<T>::NoPolkapet)?;
			// set death status
			new_pet.death = status;

			// Write updates to storage
			PolkapetsById::<T>::insert(&pet_id, new_pet);

			Ok(())
		}

		// function to set pet power
		fn update_pet_power(pet_id: PetId, new_power: u32) -> Result<(), DispatchError> {
			let mut new_pet = PolkapetsById::<T>::get(pet_id).ok_or(Error::<T>::NoPolkapet)?;
			// set new power
			new_pet.power = new_power;

			// Write updates to storage
			PolkapetsById::<T>::insert(&pet_id, new_pet);

			Ok(())
		}

		// function to generate vec from num
		fn generate_vec_reverse_digits_from_num(num: u64) -> Result<Vec<u64>, DispatchError> {
			let mut digits = Vec::<u64>::new();
			let mut n = num;

			while n > 0 {
				digits.push(n % 10);
				n = n / 10;
			}

			Ok(digits)
		}

		// Use for create random hash with seed and option u64 param
		fn generate_random_hash(seed: Option<Vec<u8>>, optional_param: Option<u64>) -> [u8; 16] {
			let random = T::MyRandomness::random(&seed.unwrap_or_default()[..]).0;
			let count = Self::last_pet_id();
			let optional_value = optional_param.unwrap_or_default();

			let unique_payload = (
				random,
				&count,
				optional_value,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			let encoded_payload = unique_payload.encode();
			let hash = (&encoded_payload).blake2_128();
			return hash
		}

		// function to return the id of a new pet and to increase 1
		fn get_new_minigame_id_and_increase() -> Result<MinigameId, DispatchError> {
			let current_last_id = &mut Self::last_minigame_id();
			let new_last_id =
				current_last_id.checked_add(1).ok_or(Error::<T>::MinigameIdOverflow)?;

			LastMinigameId::<T>::put(new_last_id);
			Ok(new_last_id)
		}

		fn operate_running_game(current_block_number: u32) -> Result<(), DispatchError> {
			let running_game_ids = RunningMinigameIds::<T>::get();
			for id in running_game_ids.iter() {
				let mut minigame = MinigameById::<T>::get(id).ok_or(Error::<T>::NoMinigame)?;
				// Calculate game resutl for each 2 block
				if current_block_number % 2 == 0 {
					Self::calculate_minigame_result(*id);
				}

				// Finish minigame
				if minigame.finish_block == current_block_number {
					// Update minigame status
					minigame.status = GameStatus::Finish;
					MinigameById::<T>::set(id, Some(minigame.clone()));

					// Remove minigame from running game list
					let mut mut_running_game_ids = RunningMinigameIds::<T>::get();
					if let Some(ind) =
						mut_running_game_ids.iter().position(|&minigame_id| minigame_id == *id)
					{
						mut_running_game_ids.swap_remove(ind);
					} else {
						return Err(Error::<T>::NoMinigame.into())
					}
					RunningMinigameIds::<T>::set(mut_running_game_ids);

					// Release pet from lock
					let mut players = MinigamePlayers::<T>::get(*id).unwrap_or_default();
					for player in players.iter_mut() {
						player.is_join_minigame = false;
						// Write updates to storage
						PolkapetsById::<T>::insert(player.pet_id, player);
					}
					MinigamePlayers::<T>::set(*id, Some(players));

					// Pay reward
					if let Some(value) = minigame.reward {
						let minigame_results =
							MinigameResultsByMinigameId::<T>::get(*id).unwrap_or_default();
						let max_point = minigame_results.last().map(|item| item.point);
						match max_point {
							Some(max_value) => {
								// Collect all items with the maximum point value
								let winner_results: Vec<&MinigameResult> = minigame_results
									.iter()
									.filter(|item| item.point == max_value)
									.collect();
								let reward_after_tax_in_u128 =
									FixedU128::saturating_from_rational(value, 100)
										.saturating_mul_int(90_u128);
								let reward_per_winner_in_u128 = reward_after_tax_in_u128
									.checked_div(winner_results.len() as u128)
									.unwrap_or_default();
								let reward_per_winner: BalanceOf<T> =
									reward_per_winner_in_u128.saturated_into::<BalanceOf<T>>();
								frame_support::log::info!(
									"======= Number of winner: {:?}.",
									winner_results.len()
								);
								frame_support::log::info!(
									"======= Reward per winner of minigame {:?} is {:?}",
									&minigame.game_id,
									&reward_per_winner
								);

								for winner_result in winner_results.iter() {
									let winner_pet = PolkapetsById::<T>::get(winner_result.pet_id)
										.ok_or(Error::<T>::NoPolkapet)?;
									// Transfer reward to winner
									T::Currency::transfer(
										&Self::account_id(),
										&winner_pet.owner,
										reward_per_winner,
										ExistenceRequirement::KeepAlive,
									)?;
								}
							},
							None => {
								frame_support::log::info!("======= No winner of minigame");
							},
						}
					} else {
						frame_support::log::info!("No Reward Game !!");
					}
				}
			}

			Ok(())
		}

		fn calculate_minigame_result(minigame_id: u64) {
			let mut minigame_results =
				MinigameResultsByMinigameId::<T>::get(minigame_id).unwrap_or_default();
			let my_string = String::from("randomResult").as_bytes().to_vec();
			for result in minigame_results.iter_mut() {
				let hash =
					Self::generate_random_hash(Some(my_string.clone()), Some(result.pet_id as u64));
				result.point = result.point + hash[0] as u32;
			}
			minigame_results.sort_by(|a, b| a.point.cmp(&b.point));
			MinigameResultsByMinigameId::<T>::set(minigame_id, Some(minigame_results));
		}
	}
}
