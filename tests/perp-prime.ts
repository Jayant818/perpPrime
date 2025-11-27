import { Address, createSolanaRpc } from "@solana/kit";
import { describe, before, test } from "node:test";

const RPC_URL = "http://127.0.0.1:8899";
const RPC_SUBSCRIPTION_URL = "ws://127.0.0.1:8900";

const PERP_PRIME_PROGRAM_ID = "" as Address;

const rpc = createSolanaRpc(RPC_URL);

