import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Klend } from "../target/types/klend";

describe("klend", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Klend as Program<Klend>;

  it("Is initialized!", async () => {
    // Add your test here.
    const tx = await program.methods.initialize().rpc();
    console.log("Your transaction signature", tx);
  });
});
