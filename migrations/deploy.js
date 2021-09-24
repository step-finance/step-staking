
// Migrations are an early feature. Currently, they're nothing more than this
// single deploy script that's invoked from the CLI, injecting a provider
// configured from the workspace's Anchor.toml.

const anchor = require("@project-serum/anchor");
const { TOKEN_PROGRAM_ID } = require("@solana/spl-token");
const fs = require('fs');

module.exports = async function (provider) {
  // Configure client to use the provider.
  anchor.setProvider(provider);

  let idl = JSON.parse(fs.readFileSync("../target/idl/step_staking.json"));
  let program = new anchor.Program(idl, "Stk5NCWomVN3itaFjLu382u9ibb5jMSHEsh6CuhaGjB", provider);

  let step = new anchor.web3.PublicKey("StepAscQoEioFxxWGnh2sLBDFp9d8rvKz2Yp39iDpyT");
  let xStep = new anchor.web3.PublicKey("xStpgUCss9piqeFUk2iLVcvJEGhAdJxJQuwLkXP555G");

  [vaultPubkey, vaultBump] =
    await anchor.web3.PublicKey.findProgramAddress(
      [step.toBuffer()],
      program.programId
    );
  
  await program.rpc.initialize(
    vaultBump,
    {
      accounts: {
        tokenMint: step,
        xTokenMint: xStep,
        tokenVault: vaultPubkey,
        initializer: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
        tokenProgram: TOKEN_PROGRAM_ID,
        rent: anchor.web3.SYSVAR_RENT_PUBKEY,
      }
    }
  );
}
