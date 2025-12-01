## Markets

| MIC | Name | Country | Currency | CSD BIC | PSET BIC |
|-----|------|---------|----------|---------|----------|
| XNYS | NYSE | US | USD | DTCYUS33 | DTCYUS33 |
| XNAS | NASDAQ | US | USD | DTCYUS33 | DTCYUS33 |
| XLON | London | GB | GBP | CABOROCP | CABOROCP |
| XFRA | Frankfurt | DE | EUR | DAKVDEFF | DAKVDEFF |
| XPAR | Euronext Paris | FR | EUR | SICVFRPP | SICVFRPP |
| XTKS | Tokyo | JP | JPY | JASDECJT | JASDECJT |
| XHKG | Hong Kong | HK | HKD | CCLOKTKT | CCLOKTKT |
| XSWX | SIX Swiss | CH | CHF | SABOROCP | SABOROCP |

## Instrument Classes

| Code | Name | Requires ISDA | Requires Market |
|------|------|---------------|-----------------|
| EQUITY | Equities | No | Yes |
| GOVT_BOND | Government Bonds | No | Yes |
| CORP_BOND | Corporate Bonds | No | Yes |
| ETF | ETFs | No | Yes |
| OTC_IRS | Interest Rate Swaps | Yes | No |
| OTC_CDS | Credit Default Swaps | Yes | No |
| OTC_FX | FX Forwards | Yes | No |

## Standard BICs (Custodians)

| Institution | BIC | Country |
|-------------|-----|---------|
| Bank of America | BOFAUS3N | US |
| Citi | CITIUS33 | US |
| JP Morgan | CHASUS33 | US |
| State Street | SBOSUS33 | US |
| BNY Mellon | IRVTUS3N | US |
| HSBC | MIDLGB22 | GB |
| Deutsche Bank | DEUTDEFF | DE |
| BNP Paribas | BNPAFRPP | FR |

## Standard BICs (Counterparties)

| Institution | BIC |
|-------------|-----|
| Morgan Stanley | MSTCUS33 |
| Goldman Sachs | GOLDUS33 |
| Barclays | BABOROCP |
| UBS | UBSWCHZH |
| Credit Suisse | CRESCHZZ |
| Deutsche Bank | DEUTDEFF |

## Settlement Types

| Code | Name | Description |
|------|------|-------------|
| DVP | Delivery vs Payment | Securities and cash exchanged simultaneously |
| FOP | Free of Payment | Securities delivered without cash leg |
| RVP | Receive vs Payment | Receive securities against payment |
| DFP | Delivery Free of Payment | Deliver securities without payment |

## SSI Types

| Code | Name | Description |
|------|------|-------------|
| SECURITIES | Securities SSI | For safekeeping and settlement |
| CASH | Cash SSI | For cash movements |
| COLLATERAL | Collateral SSI | For margin/collateral |
| FX_NOSTRO | FX Nostro | For FX settlement |

## Currencies

| Code | Name | Primary Markets |
|------|------|-----------------|
| USD | US Dollar | XNYS, XNAS |
| GBP | British Pound | XLON |
| EUR | Euro | XFRA, XPAR |
| JPY | Japanese Yen | XTKS |
| CHF | Swiss Franc | XSWX |
| HKD | Hong Kong Dollar | XHKG |

## ISDA Governing Laws

| Code | Name |
|------|------|
| NY | New York Law |
| ENGLISH | English Law |

## CSA Types

| Code | Name | Description |
|------|------|-------------|
| VM | Variation Margin | Daily margin based on MTM |
| IM | Initial Margin | Upfront collateral requirement |
