//! # EduLedger PH — Test Suite
//!
//! 5 tests covering the full MVP transaction flow for the EduLedger
//! Soroban smart contract. Tests use `Env::default()` and `mock_all_auths()`
//! to simulate a full Stellar testnet environment locally.

#[cfg(test)]
mod tests {
    use crate::{EduLedgerContract, EduLedgerContractClient};
    use soroban_sdk::{
        testutils::Address as _,
        Address, BytesN, Env, String, Symbol,
    };

    // ─────────────────────────────────────────────
    // Test Helpers
    // ─────────────────────────────────────────────

    /// Deploy the contract, initialize it with a fresh admin, and return the client + admin.
    fn setup(env: &Env) -> (EduLedgerContractClient, Address) {
        let contract_id = env.register_contract(None, EduLedgerContract);
        let client = EduLedgerContractClient::new(env, &contract_id);
        let admin = Address::generate(env);
        // Mock all auth checks so tests focus on business logic, not key management
        env.mock_all_auths();
        client.initialize(&admin);
        (client, admin)
    }

    /// Generate a deterministic 32-byte hash using a single seed byte.
    fn mock_hash(env: &Env, seed: u8) -> BytesN<32> {
        BytesN::from_array(env, &[seed; 32])
    }

    // ─────────────────────────────────────────────
    // Test 1 — Happy Path
    // ─────────────────────────────────────────────

    /// Full MVP transaction: register a transcript and verify it on-chain.
    ///
    /// Simulates the core user journey:
    /// 1. Registrar issues a transcript hash for a student
    /// 2. Employer queries the contract with the hash
    /// 3. Contract returns `true` (valid and not revoked)
    #[test]
    fn test_register_and_verify_document_happy_path() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup(&env);

        let registrar = Address::generate(&env);
        let student = Address::generate(&env);
        let doc_hash = mock_hash(&env, 0xAA);

        // Step 1: Registrar submits transcript hash to the contract
        let returned_hash = client.register_document(
            &registrar,
            &student,
            &doc_hash,
            &Symbol::new(&env, "TRANSCRIPT"),
            &String::from_str(&env, "ipfs://QmTranscriptExampleHash"),
        );

        // The returned hash must match what was submitted
        assert_eq!(returned_hash, doc_hash, "Returned hash should match input hash");

        // Step 2: Anyone verifies the document (employer, CHED, foreign university)
        let is_valid = client.verify_document(&doc_hash);

        // Step 3: Verification must succeed for a freshly registered document
        assert!(is_valid, "Newly registered document must return true on verify_document");
    }

    // ─────────────────────────────────────────────
    // Test 2 — Edge Case: Duplicate Registration
    // ─────────────────────────────────────────────

    /// Registering the exact same document hash twice must panic.
    ///
    /// Prevents a registrar from accidentally or maliciously overwriting
    /// an existing credential with different metadata.
    #[test]
    #[should_panic(expected = "Document already registered")]
    fn test_duplicate_document_registration_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup(&env);

        let registrar = Address::generate(&env);
        let student = Address::generate(&env);
        let doc_hash = mock_hash(&env, 0xBB);

        // First registration — valid
        client.register_document(
            &registrar,
            &student,
            &doc_hash,
            &Symbol::new(&env, "CREDENTIAL"),
            &String::from_str(&env, "ipfs://QmFirstRegistration"),
        );

        // Second registration with identical hash — must panic
        client.register_document(
            &registrar,
            &student,
            &doc_hash,
            &Symbol::new(&env, "CREDENTIAL"),
            &String::from_str(&env, "ipfs://QmAttemptedOverwrite"),
        );
    }

    // ─────────────────────────────────────────────
    // Test 3 — State Verification: Document Counter
    // ─────────────────────────────────────────────

    /// Verify that the document counter increments correctly after each registration.
    ///
    /// The counter is used for audit trails and dashboard analytics.
    /// It must accurately reflect the number of documents issued.
    #[test]
    fn test_document_counter_state_is_correct() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup(&env);

        let registrar = Address::generate(&env);
        let student = Address::generate(&env);

        // Counter starts at 0 after initialization
        assert_eq!(client.get_doc_count(), 0, "Initial document count should be 0");

        // Register first document (transcript)
        client.register_document(
            &registrar,
            &student,
            &mock_hash(&env, 0x01),
            &Symbol::new(&env, "TRANSCRIPT"),
            &String::from_str(&env, "ipfs://QmDoc1"),
        );
        assert_eq!(client.get_doc_count(), 1, "Count should be 1 after first registration");

        // Register second document (enrollment form)
        client.register_document(
            &registrar,
            &student,
            &mock_hash(&env, 0x02),
            &Symbol::new(&env, "ENROLLMENT"),
            &String::from_str(&env, "ipfs://QmDoc2"),
        );
        assert_eq!(client.get_doc_count(), 2, "Count should be 2 after second registration");

        // Register third document (grades)
        client.register_document(
            &registrar,
            &student,
            &mock_hash(&env, 0x03),
            &Symbol::new(&env, "GRADES"),
            &String::from_str(&env, "ipfs://QmDoc3"),
        );
        assert_eq!(client.get_doc_count(), 3, "Count should be 3 after three registrations");
    }

    // ─────────────────────────────────────────────
    // Test 4 — Edge Case: Unauthorized Revocation
    // ─────────────────────────────────────────────

    /// A third party cannot revoke a document they did not issue.
    ///
    /// Only the original issuer or contract admin may revoke.
    /// This prevents malicious actors from invalidating student credentials.
    #[test]
    #[should_panic(expected = "Unauthorized")]
    fn test_unauthorized_revocation_panics() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, _admin) = setup(&env);

        let real_registrar = Address::generate(&env);
        let student = Address::generate(&env);
        let attacker = Address::generate(&env); // completely unrelated wallet
        let doc_hash = mock_hash(&env, 0xCC);

        // Legitimate registrar issues the document
        client.register_document(
            &real_registrar,
            &student,
            &doc_hash,
            &Symbol::new(&env, "CREDENTIAL"),
            &String::from_str(&env, "ipfs://QmLegitDoc"),
        );

        // Attacker attempts to revoke a document they did not issue — must panic
        client.revoke_document(&attacker, &doc_hash);
    }

    // ─────────────────────────────────────────────
    // Test 5 — Revocation Changes On-Chain State
    // ─────────────────────────────────────────────

    /// After revocation by the admin, verify_document must return false.
    ///
    /// Full revocation lifecycle:
    /// 1. Registrar issues a document (is_valid = true)
    /// 2. Admin revokes it (e.g., fraud discovered)
    /// 3. verify_document now returns false — employer sees it as invalid
    #[test]
    fn test_revoked_document_returns_false_on_verify() {
        let env = Env::default();
        env.mock_all_auths();
        let (client, admin) = setup(&env);

        let registrar = Address::generate(&env);
        let student = Address::generate(&env);
        let doc_hash = mock_hash(&env, 0xDD);

        // Step 1: Register a credential
        client.register_document(
            &registrar,
            &student,
            &doc_hash,
            &Symbol::new(&env, "CREDENTIAL"),
            &String::from_str(&env, "ipfs://QmToBeRevoked"),
        );

        // Confirm document is valid before revocation
        assert!(
            client.verify_document(&doc_hash),
            "Document should be valid before revocation"
        );

        // Step 2: Admin revokes the document (fraud discovered post-issuance)
        client.revoke_document(&admin, &doc_hash);

        // Step 3: Same hash now returns false — employers can detect invalidity
        assert!(
            !client.verify_document(&doc_hash),
            "Revoked document must return false on verify_document"
        );
    }
}
