import * as anchor from '@coral-xyz/anchor';
import { TOKEN_PROGRAM_ID, Token } from "@solana/spl-token";
import * as utils from "./utils";
import * as assert from "assert";
import * as fs from 'fs';
import { exit } from 'process';

let program = anchor.workspace.StepStaking;

//Read the provider from the configured environment.
//represents an outside actor
//owns mints out of any other actors control, provides initial $$ to others
const envProvider = anchor.AnchorProvider.env();

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
  //hardcoded in program, read from test keys directory for testing
  let mintKey;
  let mintObject;
  let mintPubkey;
  let xMintObject;
  let xMintPubkey;

  //the program's vault for stored collateral against xToken minting
  let vaultPubkey;
  let vaultBump;

  it('Is initialized!', async () => {
    //setup logging event listeners
    program.addEventListener('PriceChange', (e, s) => {
      console.log('Price Change In Slot ', s);
      console.log('From', e.oldStepPerXstepE9.toString());
      console.log('From', e.oldStepPerXstep.toString());
      console.log('To', e.newStepPerXstepE9.toString());
      console.log('To', e.newStepPerXstep.toString());
    });

    //this already exists in ecosystem
    //test step token hardcoded in program, mint authority is wallet for testing
    let rawdata = fs.readFileSync('tests/keys/step-teST1ieLrLdr4MJPZ7i8mgSCLQ7rTrPRjNnyFdHFaz9.json');
    let keyData = JSON.parse(rawdata.toString());
    mintKey = anchor.web3.Keypair.fromSecretKey(new Uint8Array(keyData));
    mintObject = await utils.createMint(mintKey, provider, provider.wallet.publicKey, null, 9, TOKEN_PROGRAM_ID);
    mintPubkey = mintObject.publicKey;

    [vaultPubkey, vaultBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [mintPubkey.toBuffer()],
        program.programId
      );

    //this is the new xstep token
    //test xstep token hardcoded in program, mint authority is token vault
    rawdata = fs.readFileSync('tests/keys/xstep-TestZ4qmw6fCo1uK9oJbobWDgj1sME6hR1ssWQnyjxM.json');
    keyData = JSON.parse(rawdata.toString());
    let key = anchor.web3.Keypair.fromSecretKey(new Uint8Array(keyData));
    xMintObject = await utils.createMint(key, provider, vaultPubkey, null, 9, TOKEN_PROGRAM_ID);
    xMintPubkey = xMintObject.publicKey;

    [vaultPubkey, vaultBump] =
      await anchor.web3.PublicKey.findProgramAddress(
        [mintPubkey.toBuffer()],
        program.programId
      );

    await program.rpc.initialize(
      vaultBump,
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
    walletTokenAccount = await mintObject.createAssociatedTokenAccount(provider.wallet.publicKey);
    walletXTokenAccount = await xMintObject.createAssociatedTokenAccount(provider.wallet.publicKey);
    await utils.mintToAccount(provider, mintPubkey, walletTokenAccount, 100_000_000_000);
  });

  it('Swap token for xToken', async () => {
    await program.rpc.stake(
      vaultBump,
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

  it('Emit the price', async () => {
    var res = await program.simulate.emitPrice(
      {
        accounts: {
          tokenMint: mintPubkey,
          xTokenMint: xMintPubkey,
          tokenVault: vaultPubkey,
        }
      }
    )
    let price = res.events[0].data;
    console.log('Emit price: ', price.stepPerXstepE9.toString());
    console.log('Emit price: ', price.stepPerXstep.toString());
    assert.strictEqual(price.stepPerXstep.toString(), '1.2');
  });

  it('Redeem xToken for token', async () => {
    await program.rpc.unstake(
      vaultBump,
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

  it('Airdrop some tokens to the pool before xToken creation', async () => {
    await utils.mintToAccount(provider, mintPubkey, vaultPubkey, 5_000_000_000);

    assert.strictEqual(await getTokenBalance(vaultPubkey), 5_000_000_000);
  });

  it('Swap token for xToken on prefilled pool', async () => {
    await program.rpc.stake(
      vaultBump,
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
    
    assert.strictEqual(await getTokenBalance(walletTokenAccount), 96_000_000_000);
    assert.strictEqual(await getTokenBalance(walletXTokenAccount), 5_000_000_000);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 10_000_000_000);
  });

  it('Redeem xToken for token after prefilled pool', async () => {
    await program.rpc.unstake(
      vaultBump,
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

    assert.strictEqual(await getTokenBalance(walletTokenAccount), 106_000_000_000);
    assert.strictEqual(await getTokenBalance(walletXTokenAccount), 0);
    assert.strictEqual(await getTokenBalance(vaultPubkey), 0);
  });
});

async function getTokenBalance(pubkey) {
  return parseInt((await provider.connection.getTokenAccountBalance(pubkey)).value.amount);
}
