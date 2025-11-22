//! EBNF Grammar for CBU Model DSL
//!
//! Defines the formal grammar for the CBU Model specification DSL.

/// EBNF grammar for CBU Model DSL
///
/// This grammar defines the structure of a CBU Model specification,
/// which documents the business model including attributes, states,
/// transitions, and role requirements.
pub const CBU_MODEL_EBNF: &str = r#"
(* CBU Model DSL Grammar - Version 1.0 *)

(* Top-level form *)
cbu_model = "(" , "cbu-model" , model_header , model_sections , ")" ;

(* Model header with metadata *)
model_header = id_clause , version_clause , [ description_clause ] , [ applies_to_clause ] ;

id_clause = ":id" , string ;
version_clause = ":version" , string ;
description_clause = ":description" , string ;
applies_to_clause = ":applies-to" , string_list ;

(* Model sections *)
model_sections = attributes_section , states_section , transitions_section , roles_section ;

(* Attributes section - groups of required/optional attributes *)
attributes_section = "(" , "attributes" , { attribute_group } , ")" ;

attribute_group = "(" , "group" , ":name" , string ,
                  [ required_attrs ] , [ optional_attrs ] , ")" ;

required_attrs = ":required" , attr_ref_list ;
optional_attrs = ":optional" , attr_ref_list ;

(* States section - state machine definition *)
states_section = "(" , "states" ,
                 initial_state , final_states , { state_def } , ")" ;

initial_state = ":initial" , string ;
final_states = ":final" , string_list ;

state_def = "(" , "state" , string , [ state_description ] , ")" ;
state_description = ":description" , string ;

(* Transitions section - valid state transitions *)
transitions_section = "(" , "transitions" , { transition_def } , ")" ;

transition_def = "(" , "->" , string , string ,
                 ":verb" , string , [ chunks ] , [ preconditions ] , ")" ;

chunks = ":chunks" , string_list ;
preconditions = ":preconditions" , attr_ref_list ;

(* Roles section - entity role requirements *)
roles_section = "(" , "roles" , { role_def } , ")" ;

role_def = "(" , "role" , string , ":min" , number , [ max_constraint ] , ")" ;
max_constraint = ":max" , number ;

(* Primitives *)
string = '"' , { character } , '"' ;
number = digit , { digit } ;
string_list = "[" , [ string , { "," , string } ] , "]" ;
attr_ref_list = "[" , [ attr_ref , { "," , attr_ref } ] , "]" ;
attr_ref = "@attr(" , '"' , attribute_id , '"' , ")" ;
attribute_id = { character } ;  (* Dictionary attribute name or UUID *)

character = letter | digit | symbol ;
letter = "a" | "b" | ... | "z" | "A" | "B" | ... | "Z" ;
digit = "0" | "1" | ... | "9" ;
symbol = "-" | "_" | "." | "/" | ":" ;
"#;

/// Example CBU Model DSL document
pub const CBU_MODEL_EXAMPLE: &str = r#"
(cbu-model
  :id "CBU.GENERIC"
  :version "1.0"
  :description "Generic CBU for standard client onboarding"
  :applies-to ["Fund", "SPV", "Corporation"]

  (attributes
    (group :name "core"
      :required [@attr("CBU.LEGAL_NAME"), @attr("CBU.JURISDICTION"), @attr("CBU.ENTITY_TYPE")]
      :optional [@attr("CBU.TRADING_NAME"), @attr("CBU.LEI")])

    (group :name "contact"
      :required [@attr("CBU.PRIMARY_CONTACT_NAME"), @attr("CBU.PRIMARY_CONTACT_EMAIL")]
      :optional [@attr("CBU.PRIMARY_CONTACT_PHONE"), @attr("CBU.SECONDARY_CONTACT_NAME")])

    (group :name "ubo"
      :required [@attr("UBO.BENEFICIAL_OWNER_NAME"), @attr("UBO.OWNERSHIP_PERCENTAGE")]
      :optional [@attr("UBO.NATIONALITY"), @attr("UBO.TAX_RESIDENCY")]))

  (states
    :initial "Proposed"
    :final ["Closed", "Declined"]

    (state "Proposed" :description "Initial state when CBU is first created")
    (state "PendingKYC" :description "Awaiting KYC verification")
    (state "PendingApproval" :description "KYC complete, awaiting final approval")
    (state "Active" :description "CBU is fully onboarded and active")
    (state "Suspended" :description "CBU temporarily suspended")
    (state "Closed" :description "CBU relationship terminated")
    (state "Declined" :description "CBU onboarding declined"))

  (transitions
    (-> "Proposed" "PendingKYC"
        :verb "cbu.submit"
        :chunks ["core", "contact"]
        :preconditions [@attr("CBU.LEGAL_NAME"), @attr("CBU.JURISDICTION")])

    (-> "PendingKYC" "PendingApproval"
        :verb "kyc.complete"
        :chunks ["ubo"]
        :preconditions [@attr("KYC.VERIFICATION_STATUS")])

    (-> "PendingApproval" "Active"
        :verb "cbu.approve"
        :preconditions [@attr("CBU.APPROVAL_STATUS")])

    (-> "PendingApproval" "Declined"
        :verb "cbu.decline"
        :preconditions [@attr("CBU.DECLINE_REASON")])

    (-> "Active" "Suspended"
        :verb "cbu.suspend"
        :preconditions [@attr("CBU.SUSPENSION_REASON")])

    (-> "Suspended" "Active"
        :verb "cbu.reactivate"
        :preconditions [])

    (-> "Active" "Closed"
        :verb "cbu.close"
        :preconditions [@attr("CBU.CLOSURE_REASON")])

    (-> "Suspended" "Closed"
        :verb "cbu.close"
        :preconditions [@attr("CBU.CLOSURE_REASON")]))

  (roles
    (role "BeneficialOwner" :min 1 :max 10)
    (role "AuthorizedSignatory" :min 1 :max 5)
    (role "PrimaryContact" :min 1 :max 1)
    (role "ComplianceOfficer" :min 0 :max 1)))
"#;
