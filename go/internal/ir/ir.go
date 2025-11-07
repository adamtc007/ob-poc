package ir

import (
	"encoding/json"
	"time"
)

// ---------- Core IR ----------

type Plan struct {
	Version   string         `json:"version"`
	PlanID    string         `json:"plan_id"`    // uuid string
	CreatedAt time.Time      `json:"created_at"` // RFC3339
	Metadata  map[string]any `json:"metadata,omitempty"`
	Steps     []Step         `json:"steps"`
}

type Step struct {
	Op             Op              `json:"op"`
	ArgsRaw        json.RawMessage `json:"args"`
	IdempotencyKey *string         `json:"idempotency_key,omitempty"`
	Annotations    map[string]any  `json:"annotations,omitempty"`
}

// Op is the verb enum (stable API surface).
type Op string

const (
	OpInvestorStartOpportunity Op = "investor.start-opportunity"
	OpInvestorRecordIndication Op = "investor.record-indication"

	OpKYCBegin      Op = "kyc.begin"
	OpKYCCollectDoc Op = "kyc.collect-doc"
	OpKYCScreen     Op = "kyc.screen"
	OpKYCApprove    Op = "kyc.approve"

	OpTaxCapture Op = "tax.capture"

	OpBankSetInstruction Op = "bank.set-instruction"

	OpSubscribeRequest Op = "subscribe.request"
	OpCashConfirm      Op = "cash.confirm"
	OpDealNAV          Op = "deal.nav"
	OpSubscribeIssue   Op = "subscribe.issue"

	OpKYCRefreshSchedule Op = "kyc.refresh-schedule"
	OpScreenContinuous   Op = "screen.continuous"

	OpRedeemRequest Op = "redeem.request"
	OpRedeemSettle  Op = "redeem.settle"

	OpOffboardClose Op = "offboard.close"
)

// AttrRef models a late-bound attribute reference.
type AttrRef struct {
	Kind        string           `json:"kind"` // must be "AttrRef"
	ID          string           `json:"id"`
	Type        *string          `json:"type,omitempty"`
	Required    *bool            `json:"required,omitempty"`
	Constraints map[string]any   `json:"constraints,omitempty"`
	Sources     []map[string]any `json:"sources,omitempty"`
}

// ---------- Per-verb args (typed) ----------

// investor.start-opportunity
type InvestorStartOpportunityArgs struct {
	LegalName          json.RawMessage `json:"legal_name"` // string or AttrRef
	Type               string          `json:"type"`
	Domicile           json.RawMessage `json:"domicile"`                      // string or AttrRef
	LEI                json.RawMessage `json:"lei,omitempty"`                 // string or AttrRef
	RegistrationNumber json.RawMessage `json:"registration_number,omitempty"` // string or AttrRef
	Address            map[string]any  `json:"address,omitempty"`
}

// investor.record-indication
type InvestorRecordIndicationArgs struct {
	InvestorID     string  `json:"investor_id"`
	FundID         string  `json:"fund_id"`
	ClassID        string  `json:"class_id"`
	Ticket         float64 `json:"ticket"`
	Currency       *string `json:"currency,omitempty"`
	IndicationDate *string `json:"indication_date,omitempty"` // date
}

// kyc.begin
type KYCBeginArgs struct {
	InvestorID string  `json:"investor_id"`
	RiskRating *string `json:"risk_rating,omitempty"`
}

// kyc.collect-doc
type KYCCollectDocArgs struct {
	InvestorID string  `json:"investor_id"`
	DocType    string  `json:"doc_type"`
	Subject    *string `json:"subject,omitempty"`
	SubjectID  *string `json:"subject_id,omitempty"`
	URI        *string `json:"uri,omitempty"`
	SHA256     *string `json:"sha256,omitempty"`
	IssuedOn   *string `json:"issued_on,omitempty"`  // date
	ExpiresOn  *string `json:"expires_on,omitempty"` // date
}

// kyc.screen
type KYCScreenArgs struct {
	InvestorID    string  `json:"investor_id"`
	Provider      string  `json:"provider"`
	Reference     *string `json:"reference,omitempty"`
	ScreeningDate *string `json:"screening_date,omitempty"` // date
}

// kyc.approve
type KYCApproveArgs struct {
	InvestorID    string  `json:"investor_id"`
	Risk          string  `json:"risk"`
	RefreshDue    string  `json:"refresh_due"` // date
	ApprovedBy    *string `json:"approved_by,omitempty"`
	ApprovalNotes *string `json:"approval_notes,omitempty"`
	SOWSummary    *string `json:"sow_summary,omitempty"`
	SOFSummary    *string `json:"sof_summary,omitempty"`
}

// tax.capture
type TaxCaptureArgs struct {
	InvestorID      string   `json:"investor_id"`
	FATCA           string   `json:"fatca"`
	CRS             string   `json:"crs"`
	Form            string   `json:"form"`
	TIN             *string  `json:"tin,omitempty"`
	WithholdingRate *float64 `json:"withholding_rate,omitempty"`
	FormSignedDate  *string  `json:"form_signed_date,omitempty"`  // date
	FormExpiresDate *string  `json:"form_expires_date,omitempty"` // date
}

// bank.set-instruction
type BankSetInstructionArgs struct {
	InvestorID        string  `json:"investor_id"`
	Currency          string  `json:"currency"` // ISO 4217
	AccountName       *string `json:"account_name,omitempty"`
	IBAN              *string `json:"iban,omitempty"`
	AccountNo         *string `json:"account_no,omitempty"`
	SWIFTBIC          string  `json:"swift_bic"`
	IntermediaryBank  *string `json:"intermediary_bank,omitempty"`
	IntermediarySwift *string `json:"intermediary_swift,omitempty"`
	ActiveFrom        *string `json:"active_from,omitempty"` // date
	ActiveTo          *string `json:"active_to,omitempty"`   // date
}

// subscribe.request
type SubscribeRequestArgs struct {
	InvestorID string  `json:"investor_id"`
	ClassID    string  `json:"class_id"`
	SeriesID   *string `json:"series_id,omitempty"`
	Amount     float64 `json:"amount"`
	TradeDate  string  `json:"trade_date"` // date
	Currency   string  `json:"currency"`   // ISO 4217
	BankID     *string `json:"bank_id,omitempty"`
}

// cash.confirm
type CashConfirmArgs struct {
	InvestorID  string  `json:"investor_id"`
	TradeID     *string `json:"trade_id,omitempty"`
	Amount      float64 `json:"amount"`
	ValueDate   string  `json:"value_date"` // date
	BankID      *string `json:"bank_id,omitempty"`
	Reference   *string `json:"reference,omitempty"`
	ConfirmedBy *string `json:"confirmed_by,omitempty"`
}

// deal.nav
type DealNAVArgs struct {
	FundID            string   `json:"fund_id"`
	ClassID           *string  `json:"class_id,omitempty"`
	NAVDate           string   `json:"nav_date"` // date
	NAVPerShare       *float64 `json:"nav_per_share,omitempty"`
	TotalNAV          *float64 `json:"total_nav,omitempty"`
	SharesOutstanding *float64 `json:"shares_outstanding,omitempty"`
}

// subscribe.issue
type SubscribeIssueArgs struct {
	InvestorID     string   `json:"investor_id"`
	TradeID        *string  `json:"trade_id,omitempty"`
	ClassID        string   `json:"class_id"`
	SeriesID       *string  `json:"series_id,omitempty"`
	NAVPerShare    float64  `json:"nav_per_share"`
	Units          float64  `json:"units"`
	NAVDate        *string  `json:"nav_date,omitempty"`        // date
	SettlementDate *string  `json:"settlement_date,omitempty"` // date
	FeesAmount     *float64 `json:"fees_amount,omitempty"`
}

// kyc.refresh-schedule
type KYCRefreshScheduleArgs struct {
	InvestorID string `json:"investor_id"`
	Frequency  string `json:"frequency"`
	Next       string `json:"next"` // date
}

// screen.continuous
type ScreenContinuousArgs struct {
	InvestorID   string  `json:"investor_id"`
	Frequency    string  `json:"frequency"`
	Provider     *string `json:"provider,omitempty"`
	AutoEscalate *bool   `json:"auto_escalate,omitempty"`
}

// redeem.request
type RedeemRequestArgs struct {
	InvestorID string   `json:"investor_id"`
	ClassID    string   `json:"class_id"`
	SeriesID   *string  `json:"series_id,omitempty"`
	Units      float64  `json:"units"`
	Percentage *float64 `json:"percentage,omitempty"`
	NoticeDate string   `json:"notice_date"`          // date
	TradeDate  *string  `json:"trade_date,omitempty"` // date
	BankID     *string  `json:"bank_id,omitempty"`
}

// redeem.settle
type RedeemSettleArgs struct {
	InvestorID string   `json:"investor_id"`
	TradeID    *string  `json:"trade_id,omitempty"`
	Amount     float64  `json:"amount"`
	SettleDate string   `json:"settle_date"` // date
	BankID     *string  `json:"bank_id,omitempty"`
	Reference  *string  `json:"reference,omitempty"`
	FXRate     *float64 `json:"fx_rate,omitempty"`
	FeesAmount *float64 `json:"fees_amount,omitempty"`
}

// offboard.close
type OffboardCloseArgs struct {
	InvestorID           string  `json:"investor_id"`
	ClosureDate          *string `json:"closure_date,omitempty"` // date
	Reason               *string `json:"reason,omitempty"`
	FinalConfirmation    *bool   `json:"final_confirmation,omitempty"`
	RetentionPeriodYears *int    `json:"retention_period_years,omitempty"`
}
