# EduLedger PH 🎓

> **On-chain school document registry and scholarship disbursement for Philippine universities — built on Stellar with Soroban smart contracts.**

---

## Problem

A graduating student at a public university in the Philippines waits **2–6 weeks** and pays ₱500–₱2,000 in fees to get a certified true copy of their transcript — a document that employers and foreign universities still cannot trust due to rampant forgery, costing students jobs and graduate school admissions.

## Solution

The university registrar registers the SHA-256 hash of each student document (transcript, credential, enrollment form, grade record) into this Soroban contract on Stellar. Any employer can verify the document's authenticity in under 5 seconds by querying the contract with the hash — no middlemen, no waiting, no forgery possible. Scholarship funds are disbursed directly to the student's Stellar wallet in USDC.

---

## Timeline

| Phase | Milestone |
|---|---|
| Day 1–2 | Deploy contract to Stellar testnet; test `register_document` + `verify_document` |
| Day 3–4 | Implement scholarship escrow + `disburse_scholarship` |
| Day 5–6 | Build registrar web UI + student QR share page |
| Day 7 | Demo prep + testnet integration test |

---

## Stellar Features Used

| Feature | Usage |
|---|---|
| **Soroban Smart Contracts** | Document registry, grade records, scholarship escrow |
| **USDC / XLM Transfers** | Scholarship disbursement via `token::Client::transfer` |
| **Custom Tokens** | School-issued soulbound credential tokens (optional) |
| **Trustlines** | Student wallet accepts school-issued credential asset |
| **Clawback** | Revoke fraudulent credentials via `revoke_document` |

---

## Vision & Purpose

EduLedger PH eliminates the trust gap between Philippine academic institutions and the employers/universities that rely on their documents. By anchoring document hashes to Stellar's permanent, public ledger, we create a **verifiable credential layer** that costs fractions of a peso to maintain — accessible to any institution regardless of IT budget.

Long-term: expand to all Southeast Asian universities, integrate with the Philippine Statistics Authority (PSA), and enable cross-border credential recognition.

---

## Project Structure

```
edu_ledger/
├── Cargo.toml
├── README.md
├── IDEA.md                 ← Full Stellar dApp idea brief
└── src/
    ├── lib.rs              ← Soroban smart contract
    └── test.rs             ← 5 contract tests
```

---

## Prerequisites

| Tool | Version | Install |
|---|---|---|
| Rust | stable (≥ 1.74) | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Wasm target | — | `rustup target add wasm32-unknown-unknown` |
| Soroban CLI | ≥ 21.x | `cargo install --locked soroban-cli` |
| Stellar Testnet account | — | [Stellar Laboratory](https://laboratory.stellar.org/) |

---

## How to Build

```bash
# Build the Wasm binary for Stellar deployment
soroban contract build

# Output: target/wasm32-unknown-unknown/release/edu_ledger.wasm
```

---

## How to Test

```bash
# Run all 5 unit tests locally
cargo test

# Run with output for debugging
cargo test -- --nocapture
```

Expected output:
```
test test::tests::test_register_and_verify_document_happy_path ... ok
test test::tests::test_duplicate_document_registration_panics ... ok
test test::tests::test_document_counter_state_is_correct ... ok
test test::tests::test_unauthorized_revocation_panics ... ok
test test::tests::test_revoked_document_returns_false_on_verify ... ok

test result: ok. 5 passed; 0 failed
```

---

## How to Deploy to Testnet

### 1. Set up your Stellar testnet identity

```bash
soroban keys generate --global alice --network testnet
soroban keys address alice
# Fund via Friendbot: https://friendbot.stellar.org/?addr=<YOUR_ADDRESS>
```

### 2. Deploy the contract

```bash
soroban contract deploy \
  --wasm target/wasm32-unknown-unknown/release/edu_ledger.wasm \
  --source alice \
  --network testnet
# → Returns: CONTRACT_ID (save this!)
```

### 3. Initialize the contract

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- initialize \
  --admin <YOUR_STELLAR_ADDRESS>
```

---

## Sample CLI Invocations

### Register a Document (Registrar issues a transcript)

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- register_document \
  --issuer GREGISTRAR...ADDRESS \
  --student GSTUDENT...ADDRESS \
  --doc_hash aabbccdd...3232hexbytes \
  --doc_type TRANSCRIPT \
  --metadata "ipfs://QmExampleTranscriptHash"
```

### Verify a Document (Employer checks authenticity)

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- verify_document \
  --doc_hash aabbccdd...3232hexbytes
# → true
```

### Enroll a Student

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- enroll_student \
  --admin <YOUR_STELLAR_ADDRESS> \
  --student GSTUDENT...ADDRESS \
  --name "Maria Santos"
```

### Record Grades

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- record_grades \
  --issuer GREGISTRAR...ADDRESS \
  --student GSTUDENT...ADDRESS \
  --grade_hash 1122334455...3232hexbytes
```

### Create a Scholarship

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- create_scholarship \
  --issuer GFOUNDATION...ADDRESS \
  --scholarship_id 1001 \
  --recipient GSTUDENT...ADDRESS \
  --amount 50000000 \
  --token GUSDC...CONTRACT_ADDRESS
```

### Disburse Scholarship Funds

```bash
soroban contract invoke \
  --id <CONTRACT_ID> \
  --source alice \
  --network testnet \
  -- disburse_scholarship \
  --caller GFOUNDATION...ADDRESS \
  --scholarship_id 1001
```

---

## Contract Functions Reference

| Function | Auth Required | Description |
|---|---|---|
| `initialize(admin)` | admin | One-time setup |
| `register_document(...)` | issuer | Issue a document hash on-chain |
| `verify_document(doc_hash)` | None | Public: check if hash is valid |
| `get_document(doc_hash)` | None | Public: get full document record |
| `revoke_document(caller, hash)` | admin or issuer | Invalidate a document |
| `enroll_student(admin, student, name)` | admin | Create student profile |
| `record_grades(issuer, student, hash)` | issuer | Update grade hash |
| `get_student(student)` | None | Public: get student profile |
| `create_scholarship(...)` | issuer | Lock scholarship terms on-chain |
| `disburse_scholarship(caller, id)` | issuer | Transfer USDC to student |
| `get_doc_count()` | None | Public: total documents issued |

---

## License

MIT © EduLedger PH Team

Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions: The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
