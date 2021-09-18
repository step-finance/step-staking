const anchor = require("@project-serum/anchor");
const { TOKEN_PROGRAM_ID, Token } = require("@solana/spl-token");

async function createMint(provider, decimals) {
    const mint = await Token.createMint(
        provider.connection,
        provider.wallet.payer,
        provider.wallet.publicKey,
        null,
        decimals,
        TOKEN_PROGRAM_ID
    );
    return mint;
}

async function mintToAccount(
    provider,
    mint,
    destination,
    amount
) {
    const tx = new anchor.web3.Transaction();
    tx.add(
      Token.createMintToInstruction(
        TOKEN_PROGRAM_ID,
        mint,
        destination,
        provider.wallet.publicKey,
        [],
        amount
      )
    );
    await provider.send(tx);
}

async function sendLamports(
    provider,
    destination,
    amount
) {
    const tx = new anchor.web3.Transaction();
    tx.add(
        anchor.web3.SystemProgram.transfer(
            { 
                fromPubkey: provider.wallet.publicKey, 
                lamports: amount, 
                toPubkey: destination
            }
        )
    );
    await provider.send(tx);
}


module.exports = {
    createMint,
    mintToAccount,
    sendLamports,
};
