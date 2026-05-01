#![no_std]

//! # EduLedger PH — Soroban Smart Contract
//!
//! On-chain school document registry and scholarship disbursement
//! for Philippine universities built on Stellar.
//!
//! ## Core Capabilities
//! - Register & verify school documents (transcripts, credentials, enrollment, grades)
//! - Enroll students on-chain
//! - Record grade hashes with data integrity
//! - Create & disburse scholarship funds via USDC

use soroban_sdk::{
    contract, contractimpl, contracttype,
    Address, BytesN, Env, String, Symbol,
};

// ─────────────────────────────────────────────
// Storage Key Enum
// Each variant defines a unique namespace for persistent data.
// ─────────────────────────────────────────────

#[contracttype]
pub enum DataKey {
    /// Maps a SHA-256 document hash → DocumentRecord
    Document(BytesN<32>),
    /// Maps a student wallet address → StudentProfile
    Student(Address),
    /// Maps a numeric scholarship ID → ScholarshipRecord
    Scholarship(u64),
    /// Singleton: the contract administrator (registrar office)
    Admin,
    /// Singleton: running total of documents registered
    DocCounter,
}

// ─────────────────────────────────────────────
// Data Structures
// ─────────────────────────────────────────────

/// Represents a school document registered on-chain.
/// Stores metadata and validity status; actual file lives off-chain (IPFS).
#[contracttype]
#[derive(Clone)]
pub struct DocumentRecord {
    /// Stellar address of the student this document belongs to
    pub student: Address,
    /// Document category: TRANSCRIPT | CREDENTIAL | ENROLLMENT | GRADES
    pub doc_type: Symbol,
    /// Stellar address of the issuing institution (registrar office)
    pub issuer: Address,
    /// Ledger timestamp at time of registration (Unix epoch)
    pub timestamp: u64,
    /// False after revocation; used to flag fraudulent or erroneous records
    pub is_valid: bool,
    /// IPFS CID or human-readable description (e.g., "ipfs://Qm...")
    pub metadata: String,
}

/// On-chain profile for a registered student.
/// Created by the admin (registrar) during enrollment.
#[contracttype]
#[derive(Clone)]
pub struct StudentProfile {
    /// Full legal name as registered
    pub name: String,
    /// Whether the student is currently enrolled
    pub enrolled: bool,
    /// Hash of the student's latest grade record (all-zero if not yet recorded)
    pub grade_hash: BytesN<32>,
}

/// Escrow record for a scholarship award.
/// The institution locks terms on-chain; funds are sent separately.
#[contracttype]
#[derive(Clone)]
pub struct ScholarshipRecord {
    /// Stellar wallet of the scholarship recipient (student)
    pub recipient: Address,
    /// Amount in the token's base units (e.g., stroops for XLM)
    pub amount: i128,
    /// Contract address of the payment token (e.g., USDC on Stellar)
    pub token: Address,
    /// Prevents double-disbursement once funds are sent
    pub disbursed: bool,
    /// Institution or foundation that created this scholarship
    pub issuer: Address,
}

// ─────────────────────────────────────────────
// Contract Implementation
// ─────────────────────────────────────────────

#[contract]
pub struct EduLedgerContract;

#[contractimpl]
impl EduLedgerContract {
    // ── Admin Setup ──────────────────────────────────────────────────────────

    /// Initialize the contract with a designated admin (registrar office).
    ///
    /// Must be called exactly once after deployment.
    /// The admin is the only address that can enroll students.
    pub fn initialize(env: Env, admin: Address) {
        // Guard against re-initialization (one-time setup)
        if env.storage().instance().has(&DataKey::Admin) {
            panic!("Contract already initialized");
        }
        admin.require_auth();
        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::DocCounter, &0u64);
    }

    // ── Document Registry ────────────────────────────────────────────────────

    /// Register a school document on-chain by its SHA-256 hash.
    ///
    /// Called by the issuing institution (registrar) to certify a document.
    /// The actual file is stored off-chain (e.g., IPFS); only the hash
    /// is anchored to Stellar — ensuring tamper-proof verification without
    /// bloating the ledger.
    ///
    /// # Arguments
    /// - `issuer`   — Wallet of the registrar/institution (must sign)
    /// - `student`  — Wallet of the student who owns this document
    /// - `doc_hash` — SHA-256 hash of the document file (32 bytes)
    /// - `doc_type` — One of: TRANSCRIPT, CREDENTIAL, ENROLLMENT, GRADES
    /// - `metadata` — IPFS CID or description string
    ///
    /// # Returns
    /// The registered `doc_hash` — used as the shareable credential ID (QR code).
    pub fn register_document(
        env: Env,
        issuer: Address,
        student: Address,
        doc_hash: BytesN<32>,
        doc_type: Symbol,
        metadata: String,
    ) -> BytesN<32> {
        // Require a valid signature from the issuing institution
        issuer.require_auth();

        // Reject duplicates — same hash cannot be registered twice
        if env
            .storage()
            .persistent()
            .has(&DataKey::Document(doc_hash.clone()))
        {
            panic!("Document already registered");
        }

        let record = DocumentRecord {
            student,
            doc_type,
            issuer,
            // Ledger timestamp provides an immutable issuance date
            timestamp: env.ledger().timestamp(),
            is_valid: true,
            metadata,
        };

        // Persist document record; uses persistent storage so it survives ledger upgrades
        env.storage()
            .persistent()
            .set(&DataKey::Document(doc_hash.clone()), &record);

        // Increment the global document counter for analytics / audit
        let count: u64 = env
            .storage()
            .instance()
            .get(&DataKey::DocCounter)
            .unwrap_or(0);
        env.storage()
            .instance()
            .set(&DataKey::DocCounter, &(count + 1));

        // Return hash — caller uses this as the QR code credential ID
        doc_hash
    }

    /// Verify whether a document hash is valid and has not been revoked.
    ///
    /// Callable by anyone without authentication (public read).
    /// Returns `true`  → document is on-chain and valid
    /// Returns `false` → document was never registered or has been revoked
    pub fn verify_document(env: Env, doc_hash: BytesN<32>) -> bool {
        match env
            .storage()
            .persistent()
            .get::<DataKey, DocumentRecord>(&DataKey::Document(doc_hash))
        {
            Some(record) => record.is_valid,
            None => false,
        }
    }

    /// Retrieve the full metadata of a registered document.
    ///
    /// Use this to display issuer, timestamp, and student details in a UI.
    pub fn get_document(env: Env, doc_hash: BytesN<32>) -> DocumentRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Document(doc_hash))
            .expect("Document not found")
    }

    /// Revoke a previously issued document.
    ///
    /// Marks the document as invalid on-chain — it will now fail verification.
    /// Only the original issuer or the contract admin can revoke.
    /// Useful for: correcting errors, handling student expulsions, or fraud cases.
    pub fn revoke_document(env: Env, caller: Address, doc_hash: BytesN<32>) {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");

        let mut record: DocumentRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Document(doc_hash.clone()))
            .expect("Document not found");

        // Strict access control: only admin or original issuer may revoke
        if caller != admin && caller != record.issuer {
            panic!("Unauthorized: only admin or original issuer can revoke");
        }

        record.is_valid = false;
        env.storage()
            .persistent()
            .set(&DataKey::Document(doc_hash), &record);
    }

    // ── Student Management ───────────────────────────────────────────────────

    /// Enroll a new student into the on-chain registry.
    ///
    /// Only the contract admin (registrar office) may call this.
    /// This creates the student's on-chain profile that links to their documents.
    pub fn enroll_student(env: Env, admin: Address, student: Address, name: String) {
        admin.require_auth();

        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("Contract not initialized");

        // Verify the caller is actually the registered admin
        if admin != stored_admin {
            panic!("Unauthorized: caller is not the contract admin");
        }

        // Prevent double-enrollment for the same student wallet
        if env
            .storage()
            .persistent()
            .has(&DataKey::Student(student.clone()))
        {
            panic!("Student already enrolled");
        }

        let profile = StudentProfile {
            name,
            enrolled: true,
            // Initialize grade hash to all-zeros (sentinel for "no grades yet")
            grade_hash: BytesN::from_array(&env, &[0u8; 32]),
        };

        env.storage()
            .persistent()
            .set(&DataKey::Student(student), &profile);
    }

    /// Update the on-chain hash for a student's grade record.
    ///
    /// Called by a registrar/professor after finalizing grades for a semester.
    /// The actual grade sheet is stored off-chain; only its SHA-256 hash
    /// is stored here for integrity verification.
    pub fn record_grades(
        env: Env,
        issuer: Address,
        student: Address,
        grade_hash: BytesN<32>,
    ) {
        issuer.require_auth();

        let mut profile: StudentProfile = env
            .storage()
            .persistent()
            .get(&DataKey::Student(student.clone()))
            .expect("Student not found — must be enrolled first");

        // Update the grade hash with the latest submission
        profile.grade_hash = grade_hash;
        env.storage()
            .persistent()
            .set(&DataKey::Student(student), &profile);
    }

    /// Retrieve a student's on-chain profile.
    pub fn get_student(env: Env, student: Address) -> StudentProfile {
        env.storage()
            .persistent()
            .get(&DataKey::Student(student))
            .expect("Student not found")
    }

    // ── Scholarship Escrow ───────────────────────────────────────────────────

    /// Create a scholarship escrow record on-chain.
    ///
    /// The issuing institution commits scholarship terms (recipient, amount, token)
    /// to the ledger. The actual tokens must be sent to this contract separately
    /// before disbursement can occur.
    ///
    /// # Arguments
    /// - `scholarship_id` — Unique numeric ID (institution's internal reference)
    /// - `recipient`      — Student's Stellar wallet address
    /// - `amount`         — Token amount in base units (e.g., 10_000_000 = 1 USDC)
    /// - `token`          — Address of the payment token contract
    pub fn create_scholarship(
        env: Env,
        issuer: Address,
        scholarship_id: u64,
        recipient: Address,
        amount: i128,
        token: Address,
    ) {
        issuer.require_auth();

        // Prevent overwriting an existing scholarship record
        if env
            .storage()
            .persistent()
            .has(&DataKey::Scholarship(scholarship_id))
        {
            panic!("Scholarship ID already exists");
        }

        let record = ScholarshipRecord {
            recipient,
            amount,
            token,
            disbursed: false,
            issuer,
        };

        env.storage()
            .persistent()
            .set(&DataKey::Scholarship(scholarship_id), &record);
    }

    /// Disburse scholarship funds to the student recipient.
    ///
    /// Transfers the agreed token amount from this contract's balance
    /// to the student's wallet using Stellar's native token interface.
    ///
    /// Stellar's low fees (~0.00001 XLM) and 5-second finality make this
    /// far superior to traditional bank wire or check-based disbursements.
    pub fn disburse_scholarship(env: Env, caller: Address, scholarship_id: u64) {
        caller.require_auth();

        let mut record: ScholarshipRecord = env
            .storage()
            .persistent()
            .get(&DataKey::Scholarship(scholarship_id))
            .expect("Scholarship not found");

        // Guard against double-disbursement attacks
        if record.disbursed {
            panic!("Scholarship already disbursed");
        }

        // Only the institution that created the scholarship can trigger it
        if caller != record.issuer {
            panic!("Unauthorized: only the scholarship issuer can disburse");
        }

        // Use Stellar's token interface to transfer USDC/XLM from this contract
        let token_client = soroban_sdk::token::Client::new(&env, &record.token);
        token_client.transfer(
            &env.current_contract_address(), // from: contract holds the funds
            &record.recipient,               // to: student's wallet
            &record.amount,                  // amount: locked-in scholarship value
        );

        // Mark disbursed to prevent future claims
        record.disbursed = true;
        env.storage()
            .persistent()
            .set(&DataKey::Scholarship(scholarship_id), &record);
    }

    /// Get a scholarship record by ID.
    pub fn get_scholarship(env: Env, scholarship_id: u64) -> ScholarshipRecord {
        env.storage()
            .persistent()
            .get(&DataKey::Scholarship(scholarship_id))
            .expect("Scholarship not found")
    }

    // ── Analytics ────────────────────────────────────────────────────────────

    /// Returns the total number of documents registered on this contract.
    /// Useful for dashboard displays and audit trails.
    pub fn get_doc_count(env: Env) -> u64 {
        env.storage()
            .instance()
            .get(&DataKey::DocCounter)
            .unwrap_or(0)
    }
}

// Include test module (only compiled during testing)
#[cfg(test)]
mod test;
