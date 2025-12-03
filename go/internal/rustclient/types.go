// Package rustclient provides types and HTTP client for the Rust DSL API.
package rustclient

import (
	"time"

	"github.com/google/uuid"
)

// CbuGraph is the graph projection of a CBU for visualization.
type CbuGraph struct {
	CbuID        uuid.UUID   `json:"cbu_id"`
	Label        string      `json:"label"`
	CbuCategory  *string     `json:"cbu_category,omitempty"`
	Jurisdiction *string     `json:"jurisdiction,omitempty"`
	Nodes        []GraphNode `json:"nodes"`
	Edges        []GraphEdge `json:"edges"`
	Layers       []LayerInfo `json:"layers"`
	Stats        GraphStats  `json:"stats"`
}

// GraphNode represents a node in the graph.
type GraphNode struct {
	ID           string     `json:"id"`
	NodeType     NodeType   `json:"node_type"`
	Layer        LayerType  `json:"layer"`
	Label        string     `json:"label"`
	Sublabel     *string    `json:"sublabel,omitempty"`
	Status       NodeStatus `json:"status"`
	Data         any        `json:"data"`
	ParentID     *string    `json:"parent_id,omitempty"`
	Roles        []string   `json:"roles,omitempty"`
	PrimaryRole  *string    `json:"primary_role,omitempty"`
	Jurisdiction *string    `json:"jurisdiction,omitempty"`
	RolePriority *int       `json:"role_priority,omitempty"`
}

// NodeType enumerates node types.
type NodeType string

const (
	NodeTypeCbu           NodeType = "cbu"
	NodeTypeMarket        NodeType = "market"
	NodeTypeUniverse      NodeType = "universe"
	NodeTypeSsi           NodeType = "ssi"
	NodeTypeBookingRule   NodeType = "booking_rule"
	NodeTypeIsda          NodeType = "isda"
	NodeTypeCsa           NodeType = "csa"
	NodeTypeSubcustodian  NodeType = "subcustodian"
	NodeTypeDocument      NodeType = "document"
	NodeTypeAttribute     NodeType = "attribute"
	NodeTypeVerification  NodeType = "verification"
	NodeTypeEntity        NodeType = "entity"
	NodeTypeOwnershipLink NodeType = "ownership_link"
	NodeTypeProduct       NodeType = "product"
	NodeTypeService       NodeType = "service"
	NodeTypeResource      NodeType = "resource"
)

// LayerType categorizes nodes into layers.
type LayerType string

const (
	LayerTypeCore     LayerType = "core"
	LayerTypeCustody  LayerType = "custody"
	LayerTypeKyc      LayerType = "kyc"
	LayerTypeUbo      LayerType = "ubo"
	LayerTypeServices LayerType = "services"
)

// NodeStatus represents node status.
type NodeStatus string

const (
	NodeStatusActive    NodeStatus = "active"
	NodeStatusPending   NodeStatus = "pending"
	NodeStatusSuspended NodeStatus = "suspended"
	NodeStatusExpired   NodeStatus = "expired"
	NodeStatusDraft     NodeStatus = "draft"
)

// GraphEdge connects two nodes.
type GraphEdge struct {
	ID       string   `json:"id"`
	Source   string   `json:"source"`
	Target   string   `json:"target"`
	EdgeType EdgeType `json:"edge_type"`
	Label    *string  `json:"label,omitempty"`
}

// EdgeType represents edge types.
type EdgeType string

const (
	EdgeTypeHasRole   EdgeType = "has_role"
	EdgeTypeRoutesTo  EdgeType = "routes_to"
	EdgeTypeMatches   EdgeType = "matches"
	EdgeTypeCoveredBy EdgeType = "covered_by"
	EdgeTypeSecuredBy EdgeType = "secured_by"
	EdgeTypeSettlesAt EdgeType = "settles_at"
	EdgeTypeRequires  EdgeType = "requires"
	EdgeTypeValidates EdgeType = "validates"
	EdgeTypeOwns      EdgeType = "owns"
	EdgeTypeControls  EdgeType = "controls"
	EdgeTypeDelivers  EdgeType = "delivers"
	EdgeTypeBelongsTo EdgeType = "belongs_to"
)

// LayerInfo for UI rendering.
type LayerInfo struct {
	LayerType LayerType `json:"layer_type"`
	Label     string    `json:"label"`
	Color     string    `json:"color"`
	NodeCount int       `json:"node_count"`
	Visible   bool      `json:"visible"`
}

// GraphStats contains graph statistics.
type GraphStats struct {
	TotalNodes   int            `json:"total_nodes"`
	TotalEdges   int            `json:"total_edges"`
	NodesByLayer map[string]int `json:"nodes_by_layer"`
	NodesByType  map[string]int `json:"nodes_by_type"`
}

// CbuSummary for list views.
type CbuSummary struct {
	CbuID        uuid.UUID  `json:"cbu_id"`
	Name         string     `json:"name"`
	Jurisdiction *string    `json:"jurisdiction,omitempty"`
	ClientType   *string    `json:"client_type,omitempty"`
	CreatedAt    *time.Time `json:"created_at,omitempty"`
	UpdatedAt    *time.Time `json:"updated_at,omitempty"`
}
