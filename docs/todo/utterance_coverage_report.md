# Utterance Coverage Report (API Execute Path)

- Total utterances: 134
- Pass (top1 == expected or in alt_verbs): 52
- Fail: 82
- Accuracy: 38.81%

## Accuracy by category
- adversarial: 2/7 (28.57%)
- direct: 21/31 (67.74%)
- indirect: 1/21 (4.76%)
- multi_intent: 0/1 (0.00%)
- natural: 28/74 (37.84%)

## Accuracy by difficulty
- easy: 22/34 (64.71%)
- expert: 3/20 (15.00%)
- hard: 4/27 (14.81%)
- medium: 23/53 (43.40%)

## Accuracy by entity prefix (from expected verb)
- agent: 2/5 (40.00%)
- billing: 0/1 (0.00%)
- bods: 1/1 (100.00%)
- booking-location: 1/1 (100.00%)
- booking-principal: 1/2 (50.00%)
- cbu: 7/21 (33.33%)
- client-group: 7/7 (100.00%)
- contract: 0/1 (0.00%)
- control: 2/3 (66.67%)
- deal: 4/12 (33.33%)
- document: 2/10 (20.00%)
- entity: 2/9 (22.22%)
- fund: 4/10 (40.00%)
- gleif: 4/7 (57.14%)
- ownership: 4/6 (66.67%)
- screening: 3/13 (23.08%)
- session: 3/7 (42.86%)
- ubo: 2/14 (14.29%)
- view: 3/4 (75.00%)

## First 40 mismatches
- #106: expected `document.verify` got `allegation.verify` | verify it
- #107: expected `screening.sanctions` got `discovery.run` | run the check
- #109: expected `gleif.import-tree` got `bods.import` | import the data
- #110: expected `cbu.create` got `trading-profile.create-new-version` | create a new one
- #108: expected `ubo.list-owners` got `cbu.show` | show the structure
- #5: expected `cbu.list` got `cbu.show` | show me all the CBUs
- #17: expected `entity.create-limited-company` got `legal-entity.create` | create a legal entity for HSBC Holdings plc
- #22: expected `entity.list` got `cbu.show` | show me all entities
- #40: expected `ubo.calculate` got `manco.group.for-cbu` | calculate the UBOs for this CBU
- #46: expected `fund.create-umbrella` got `None` | create an umbrella fund — UCITS, Luxembourg domiciled
- #58: expected `document.upload-version` got `contract.for-client` | upload the passport for John Smith
- #74: expected `deal.cancel` got `None` | cancel this deal — client withdrew
- #96: expected `session.load-galaxy` got `cbu.show` | show me the galaxy for Germany
- #123: expected `agent.teach` got `cbu.create` | teach the system that 'spin up' means create CBU
- #55: expected `fund.create-standalone` got `None` | create a standalone fund — AIF, Jersey
- #34: expected `screening.adverse-media` got `entity.delete` | has anyone flagged this entity before?
- #42: expected `ubo.mark-deceased` got `board.list-by-person` | this person died last month
- #43: expected `ubo.add-trust-role` got `trading-profile.create-new-version` | the settlor appointed a new trustee
- #112: expected `screening.sanctions` got `discovery.run` | run the RBA assessment
- #114: expected `cbu.add-product` got `service-availability.set` | set up the NAV calc service
- #115: expected `cbu.add-product` got `fund.add-investment` | add the TA and fund accounting
- #116: expected `ubo.calculate` got `coverage.compute` | compute the 25% threshold test
- #117: expected `ownership.reconcile` got `ownership.compute` | flag the circular ownership
- #120: expected `screening.sanctions` got `document.for-entity` | KYC refresh for this entity
- #121: expected `ubo.trace-chains` got `None` | nominee structure — need to pierce the veil
- #122: expected `document.for-entity` got `contract.for-client` | where's the CDD pack for this client?
- #6: expected `cbu.list` got `movement.transfer-in` | what clients do we have in Germany?
- #7: expected `cbu.list` got `onboarding.auto-complete` | how many onboarding cases are open right now?
- #14: expected `cbu.read` got `cbu.create` | what products does this CBU have?
- #23: expected `entity.list-placeholders` got `semantic.missing-entities` | which entities haven't been verified yet?
- #45: expected `ubo.update-ownership` got `ownership.right.add-to-class` | ownership was transferred from Parent Co to New Holdco
- #62: expected `document.catalog` got `case-type.deactivate` | what type of documents do we collect for limited companies?
- #71: expected `deal.counter-rate-card` got `gleif.import-to-client-group` | the client wants to counter our pricing
- #97: expected `session.set-persona` got `view.back-to` | I want to work as a compliance officer
- #131: expected `deal.list-rate-card-lines` got `semantic.stages-for-product` | what's the fee for custody on this product?
- #30: expected `screening.sanctions` got `None` | run a full screening — sanctions, PEP, and adverse media
- #51: expected `fund.list-subfunds` got `fund.create-umbrella` | what subfunds are in this umbrella?
- #88: expected `view.universe` got `cbu.show` | show me everything
- #125: expected `agent.list-tools` got `None` | what tools do you have?
- #118: expected `screening.sanctions` got `regulatory.status.check` | check for OFAC hits
