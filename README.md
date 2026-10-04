# Solana program

- Program id: `B4shPJRpKJx5Cy3nfm8Kgw6LaSDibytkpxLGqus1R5D9` (Solana mainnet)

## Verifiable build

```bash
solana-verify build --library-name raidpad
solana-verify get-executable-hash target/deploy/raidpad.so
solana-verify get-program-hash -um B4shPJRpKJx5Cy3nfm8Kgw6LaSDibytkpxLGqus1R5D9
```

The two hashes must be equal. The same build runs in this repository's "Verifiable build" workflow.
