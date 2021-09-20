const anchor = require('@project-serum/anchor');
const { TOKEN_PROGRAM_ID, Token } = require("@solana/spl-token");
const utils = require("./utils");
const assert = require("assert");

let program = anchor.workspace.StepStaking;

//Read the provider from the configured environmnet.
//represents an outside actor
//owns mints out of any other actors control, provides initial $$ to others
const envProvider = anchor.Provider.env();

//we allow this convenience var to change between default env and mock user(s)
//initially we are the outside actor
let provider = envProvider;
//convenience method to set in anchor AND above convenience var
//setting in anchor allows the rpc and accounts namespaces access
//to a different wallet from env
function setProvider(p) {
  provider = p;
  anchor.setProvider(p);
  program = new anchor.Program(program.idl, program.programId, p);
};
setProvider(provider);

describe('step-staking', () => {

  //any existing ecosystem token
  let mintObject;
  let mintPubkey;

  //the ecosystems corresponding xToken
  let xMintObject;
  let xMintPubkey;
  let mintBump;

  //the program's vault for stored collateral against xToken minting
  let vaultPubkey;
  let vaultBump;

  it('Is initialized!', async () => {
    mintObject = await utils.createMint(provider, 9);
    mintPubkey = mintObject.publicKey;

    [xMintPubkey, mintBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("mint"), mintPubkey.toBuffer()],
        program.programId
      );
    [vaultPubkey, vaultBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [Buffer.from("vault"), mintPubkey.toBuffer()],
        program.programId
      );

    await program.rpc.initializeXMint(
      {
        accounts: {
          tokenMint: mintPubkey,
          xTokenMint: xMintPubkey,
          tokenVault: vaultPubkey,
          initializer: provider.wallet.publicKey,
          systemProgram: anchor.web3.SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: anchor.web3.SYSVAR_RENT_PUBKEY,
        }
      }
    );
  });

  let walletTokenAccount;
  let walletXTokenAccount;

  it('Mint test tokens', async () => {
    xMintObject = new Token(provider.connection, xMintPubkey, TOKEN_PROGRAM_ID, provider.wallet.payer);

    walletTokenAccount = await mintObject.createAssociatedTokenAccount(provider.wallet.publicKey);
    walletXTokenAccount = await xMintObject.createAssociatedTokenAccount(provider.wallet.publicKey);
    await utils.mintToAccount(provider, mintPubkey, walletTokenAccount, 100_000_000_000);
  });

  it('Swap token for xToken', async () => {
    await program.rpc.enter(
      mintBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          xTokenMint: xMintPubkey,
          tokenFrom: walletTokenAccount,
          tokenFromAuthority: provider.wallet.publicKey,
          tokenVault: vaultPubkey,
          xTokenTo: walletXTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        }
      }
    );

    assert.strictEqual(await getTokenBalance(walletTokenAccount), 95_000_000_000);
    assert.strictEqual(await getTokenBalance(walletXTokenAccount), 5_000_000_000);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 5_000_000_000);
  });

  it('Airdrop some tokens to the pool', async () => {
    await utils.mintToAccount(provider, mintPubkey, vaultPubkey, 1_000_000_000);

    assert.strictEqual(await getTokenBalance(walletTokenAccount), 95_000_000_000);
    assert.strictEqual(await getTokenBalance(walletXTokenAccount), 5_000_000_000);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 6_000_000_000);
  });

  it('Redeem xToken for token', async () => {
    await program.rpc.exit(
      mintBump,
      new anchor.BN(5_000_000_000),
      {
        accounts: {
          tokenMint: mintPubkey,
          xTokenMint: xMintPubkey,
          xTokenFrom: walletXTokenAccount,
          xTokenFromAuthority: provider.wallet.publicKey,
          tokenVault: vaultPubkey,
          tokenTo: walletTokenAccount,
          tokenProgram: TOKEN_PROGRAM_ID,
        }
      }
    );

    assert.strictEqual(await getTokenBalance(walletTokenAccount), 101_000_000_000);
    assert.strictEqual(await getTokenBalance(walletXTokenAccount), 0);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 0);
  });

  async function getTokenBalance(pubkey) {
    return parseInt((await provider.connection.getTokenAccountBalance(pubkey)).value.amount);
  }

});