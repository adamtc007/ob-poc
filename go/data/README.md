# Mock Data for Disconnected Testing

This directory contains JSON mock data files that mirror the database schema, enabling disconnected testing without requiring a live PostgreSQL connection.

## Usage

### Using Mock Store in Tests

```go
import "dsl-ob-poc/internal/mocks"

// Create mock store pointing to JSON data
mockStore := mocks.NewMockStore("path/to/data/mocks")
defer mockStore.Close()

// Use exactly like a real store
cbus, err := mockStore.ListCBUs(ctx)
services, err := mockStore.GetServicesForProducts(ctx, []string{"CUSTODY"})
```

### Exporting Real Data to JSON

If you have a live database with data, you can export it:

```bash
# Export existing database records to mock JSON files
./dsl-poc export-mock-data

# Export to custom directory
./dsl-poc export-mock-data --dir=custom/path
```

## JSON Files

- `cbus.json` - Client Business Units
- `roles.json` - Entity roles within CBUs
- `entity_types.json` - Entity type definitions
- `entities.json` - Central entity registry
- `entity_limited_companies.json` - Limited company details
- `entity_partnerships.json` - Partnership details
- `entity_proper_persons.json` - Individual person details
- `cbu_entity_roles.json` - CBU-entity-role relationships
- `products.json` - Product catalog
- `services.json` - Service catalog
- `prod_resources.json` - Production resources
- `product_services.json` - Product-service relationships
- `service_resources.json` - Service-resource relationships
- `dictionary.json` - Attribute definitions
- `attribute_values.json` - Runtime attribute values
- `dsl_ob.json` - DSL records (event sourcing)

## Mock Data Features

- **Complete Schema Coverage**: All database tables represented
- **Realistic Relationships**: Proper foreign key references between entities
- **Entity Relationship Model**: CBU → Entity Roles → Entities → Entity Types
- **DSL Evolution**: Sample DSL progression through all 7 states
- **Attribute Values**: Runtime values with source metadata

## Disconnected Testing Benefits

- **Fast Tests**: No database connection required
- **Reproducible**: Consistent data across test runs
- **Portable**: Works without PostgreSQL installation
- **Version Control**: Mock data can be versioned with code
- **CI/CD Friendly**: No external dependencies in tests

## Implementation Notes

- JSON structs use proper field tags for Go unmarshaling
- Date fields use RFC3339 format for proper parsing
- Mock store implements same interface as real store
- Relationship tables properly link entities
- JSONB fields stored as JSON strings