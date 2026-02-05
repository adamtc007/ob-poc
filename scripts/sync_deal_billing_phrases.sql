-- Sync deal and billing verb invocation_phrases to yaml_intent_patterns
-- Run after adding new deal/billing verbs

-- Deal verbs
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['create a deal', 'start a new deal', 'open deal record', 'new client deal'] WHERE full_name = 'deal.create';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['search deals', 'find deals', 'lookup deal', 'deal search'] WHERE full_name = 'deal.search';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['get deal', 'show deal', 'deal details', 'view deal'] WHERE full_name = 'deal.get';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['update deal status', 'change deal status', 'advance deal', 'move deal to'] WHERE full_name = 'deal.update-status';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['add participant', 'add team member', 'add deal participant'] WHERE full_name = 'deal.add-participant';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove participant', 'remove team member', 'remove deal participant'] WHERE full_name = 'deal.remove-participant';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list participants', 'show participants', 'deal team'] WHERE full_name = 'deal.list-participants';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['link contract', 'add contract to deal', 'attach contract'] WHERE full_name = 'deal.add-contract';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove contract', 'unlink contract', 'detach contract'] WHERE full_name = 'deal.remove-contract';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list deal contracts', 'show contracts', 'deal contracts'] WHERE full_name = 'deal.list-contracts';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['create rate card', 'new rate card', 'start rate card', 'add pricing'] WHERE full_name = 'deal.create-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['add rate card line', 'add fee line', 'add pricing line'] WHERE full_name = 'deal.add-rate-card-line';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['update rate card line', 'change fee line', 'modify pricing'] WHERE full_name = 'deal.update-rate-card-line';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove rate card line', 'delete fee line', 'remove pricing line'] WHERE full_name = 'deal.remove-rate-card-line';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['propose rate card', 'submit rate card', 'send pricing proposal'] WHERE full_name = 'deal.propose-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['counter offer', 'client counter', 'negotiate rate card'] WHERE full_name = 'deal.counter-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['revise rate card', 'update rate card', 'modify pricing'] WHERE full_name = 'deal.revise-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['agree rate card', 'accept rate card', 'finalize pricing'] WHERE full_name = 'deal.agree-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['reject rate card', 'decline rate card', 'refuse pricing'] WHERE full_name = 'deal.reject-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['cancel rate card', 'withdraw rate card', 'abort pricing'] WHERE full_name = 'deal.cancel-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['get rate card', 'show rate card', 'view pricing'] WHERE full_name = 'deal.get-rate-card';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list rate cards', 'show all rate cards', 'deal pricing history'] WHERE full_name = 'deal.list-rate-cards';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['add sla', 'add service level', 'create sla'] WHERE full_name = 'deal.add-sla';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove sla', 'delete sla', 'drop service level'] WHERE full_name = 'deal.remove-sla';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list slas', 'show slas', 'deal service levels'] WHERE full_name = 'deal.list-slas';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['add document', 'attach document', 'upload deal document'] WHERE full_name = 'deal.add-document';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove document', 'delete document', 'detach deal document'] WHERE full_name = 'deal.remove-document';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list documents', 'show documents', 'deal documents'] WHERE full_name = 'deal.list-documents';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['link ubo assessment', 'add kyc case', 'attach ubo'] WHERE full_name = 'deal.link-ubo-assessment';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list ubo assessments', 'show kyc cases', 'deal kyc'] WHERE full_name = 'deal.list-ubo-assessments';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['create onboarding request', 'start onboarding', 'handoff to onboarding'] WHERE full_name = 'deal.create-onboarding-request';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['approve onboarding request', 'accept onboarding', 'confirm onboarding'] WHERE full_name = 'deal.approve-onboarding-request';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['reject onboarding request', 'decline onboarding', 'deny onboarding'] WHERE full_name = 'deal.reject-onboarding-request';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list onboarding requests', 'show onboarding', 'pending onboarding'] WHERE full_name = 'deal.list-onboarding-requests';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['deal summary', 'deal overview', 'full deal view'] WHERE full_name = 'deal.summary';

-- Billing verbs
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['create billing profile', 'new billing profile', 'set up billing'] WHERE full_name = 'billing.create-profile';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['get billing profile', 'show billing profile', 'billing details'] WHERE full_name = 'billing.get-profile';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list billing profiles', 'show all billing', 'billing profiles'] WHERE full_name = 'billing.list-profiles';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['activate billing', 'enable billing', 'start billing'] WHERE full_name = 'billing.activate-profile';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['suspend billing', 'pause billing', 'hold billing'] WHERE full_name = 'billing.suspend-profile';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['close billing', 'end billing', 'terminate billing'] WHERE full_name = 'billing.close-profile';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['add account target', 'add cbu to billing', 'bill cbu'] WHERE full_name = 'billing.add-account-target';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['remove account target', 'remove cbu from billing', 'unbill cbu'] WHERE full_name = 'billing.remove-account-target';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['list account targets', 'show billed cbus', 'billing targets'] WHERE full_name = 'billing.list-account-targets';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['create billing period', 'open billing period', 'new billing cycle'] WHERE full_name = 'billing.create-period';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['calculate billing', 'run billing', 'compute fees'] WHERE full_name = 'billing.calculate-period';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['review billing period', 'check billing', 'audit fees'] WHERE full_name = 'billing.review-period';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['approve billing', 'confirm billing', 'accept fees'] WHERE full_name = 'billing.approve-period';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['dispute billing', 'challenge billing', 'reject fees'] WHERE full_name = 'billing.dispute-period';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['generate invoice', 'create invoice', 'bill client'] WHERE full_name = 'billing.generate-invoice';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['billing period summary', 'period details', 'fee breakdown'] WHERE full_name = 'billing.period-summary';
UPDATE "ob-poc".dsl_verbs SET yaml_intent_patterns = ARRAY['revenue summary', 'revenue report', 'billing analytics'] WHERE full_name = 'billing.revenue-summary';

-- Verify
SELECT COUNT(*) as synced_count FROM "ob-poc".dsl_verbs
WHERE domain IN ('deal', 'billing') AND yaml_intent_patterns IS NOT NULL;
