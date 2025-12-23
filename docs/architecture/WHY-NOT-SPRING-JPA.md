# Why Not Spring/JPA? A Technical Deep-Dive

## The Proposal You'll Hear

> "We should build this properly in Spring Boot with JPA/Hibernate. We have 
> the expertise, the tooling, and it's the enterprise standard. This Rust DSL 
> thing is a prototype—let's rewrite it correctly."

This document explains why that path leads to a 12-18 month project that 
delivers less functionality with more maintenance burden.

**Note:** All Spring/JPA examples use current best practices as of 2024:
- Spring Boot 3.x with Jakarta EE (not javax)
- Hibernate 6.x
- Constructor injection (not field injection)
- Lombok for boilerplate reduction
- Java 17+ records for DTOs where appropriate
- MapStruct for compile-time mapping
- Spring Data JPA with custom repository implementations

This is not a strawman. These are the patterns you'd use today.

---

## TL;DR: The Numbers

| Metric | Spring/JPA Estimate | ob-poc Actual |
|--------|---------------------|---------------|
| Lines of code | 100,000+ | 29,000 |
| Entity classes | 90+ (even with Lombok) | 0 (schema + YAML) |
| Repository interfaces | 90+ | 0 (generic executor) |
| Service classes | 60+ | 15 custom ops |
| Test startup time | 15-45 seconds | <1 second |
| Time to add new entity | 2-4 hours | 30 minutes |
| Time to add new operation | 1-2 hours | 15 minutes |
| Configuration changes | Redeploy | Restart |
| AI can generate correctly | No | Yes |

---

## 1. The Entity Class Reality (With Lombok)

### Modern Spring/JPA With All The Helpers

Even with Lombok, you still need:

```java
@Entity
@Table(name = "cbus", schema = "ob_cbu")
@EntityListeners(AuditingEntityListener.class)
@Getter
@Setter
@NoArgsConstructor
@AllArgsConstructor
@Builder
@EqualsAndHashCode(of = "cbuId")
@ToString(exclude = {"entityRoles", "productSubscriptions", "kycCases"})
public class Cbu {
    
    @Id
    @GeneratedValue(strategy = GenerationType.UUID)
    @Column(name = "cbu_id", updatable = false, nullable = false)
    private UUID cbuId;
    
    @Column(name = "cbu_name", nullable = false, length = 255)
    @NotBlank(message = "CBU name is required")
    @Size(max = 255, message = "CBU name cannot exceed 255 characters")
    private String name;
    
    @Column(name = "jurisdiction", nullable = false, length = 2)
    @NotBlank(message = "Jurisdiction is required")
    @Pattern(regexp = "^[A-Z]{2}$", message = "Jurisdiction must be ISO 3166-1 alpha-2")
    private String jurisdiction;
    
    @Enumerated(EnumType.STRING)
    @Column(name = "client_type", nullable = false)
    private ClientType clientType;
    
    @Enumerated(EnumType.STRING)
    @Column(name = "status", nullable = false)
    private CbuStatus status;
    
    @CreatedDate
    @Column(name = "created_at", nullable = false, updatable = false)
    private Instant createdAt;
    
    @CreatedBy
    @Column(name = "created_by", updatable = false)
    private UUID createdBy;
    
    @LastModifiedDate
    @Column(name = "updated_at")
    private Instant updatedAt;
    
    @LastModifiedBy
    @Column(name = "updated_by")
    private UUID updatedBy;
    
    @Version
    private Long version;
    
    // Relationships - these CANNOT be simplified with Lombok
    @OneToMany(mappedBy = "cbu", cascade = CascadeType.ALL, orphanRemoval = true)
    @BatchSize(size = 20)
    @Builder.Default
    private Set<CbuEntityRole> entityRoles = new HashSet<>();
    
    @OneToMany(mappedBy = "cbu", cascade = CascadeType.ALL, orphanRemoval = true)
    @BatchSize(size = 10)
    @Builder.Default
    private Set<CbuProductSubscription> productSubscriptions = new HashSet<>();
    
    @OneToMany(mappedBy = "cbu", cascade = CascadeType.ALL, orphanRemoval = true)
    @Builder.Default
    private Set<KycCase> kycCases = new HashSet<>();
    
    @OneToMany(mappedBy = "cbu", cascade = CascadeType.ALL, orphanRemoval = true)
    @Builder.Default
    private Set<CbuSsi> ssis = new HashSet<>();
    
    @OneToMany(mappedBy = "cbu", cascade = CascadeType.ALL, orphanRemoval = true)
    @Builder.Default
    private Set<CbuInstrumentUniverse> instrumentUniverse = new HashSet<>();
    
    // Helper methods for bidirectional relationship management
    public void addEntityRole(CbuEntityRole role) {
        entityRoles.add(role);
        role.setCbu(this);
    }
    
    public void removeEntityRole(CbuEntityRole role) {
        entityRoles.remove(role);
        role.setCbu(null);
    }
    
    public void addProductSubscription(CbuProductSubscription subscription) {
        productSubscriptions.add(subscription);
        subscription.setCbu(this);
    }
    
    // ... more helper methods for each relationship
}
```

**Lombok saved maybe 50 lines of getters/setters.** You still have:
- 25+ annotations to configure correctly
- Relationship mappings that can't be simplified
- Bidirectional relationship helper methods
- Careful exclusions in `@ToString` to avoid infinite loops
- Careful `@EqualsAndHashCode` to avoid loading lazy collections

**That's still 100+ lines per entity. Times 90 entities = 9,000+ lines minimum.**

And each annotation is a decision:
- `CascadeType.ALL` or just `PERSIST`? 
- `orphanRemoval = true`? 
- `@BatchSize(size = ?)`?
- Which `FetchType`?

Get any of these wrong → runtime bugs that are hell to debug.

### The ob-poc Equivalent

```yaml
# config/verbs/cbu.yaml - complete definition
create:
  description: "Create a new Client Business Unit"
  behavior: crud
  crud:
    operation: insert
    table: cbus
    schema: ob_cbu
  args:
    - name: name
      type: string
      required: true
      column: cbu_name
    - name: jurisdiction
      type: string
      required: true
      lookup:
        table: jurisdictions
        search_key: code
    - name: client-type
      type: string
      required: false
      default: "CORPORATE"
      column: client_type
```

**30 lines. No annotations. No relationship mapping. No cascade decisions.**

---

## 2. The Repository Layer (With Spring Data)

### Modern Spring Data JPA

```java
@Repository
public interface CbuRepository extends JpaRepository<Cbu, UUID>, 
                                       JpaSpecificationExecutor<Cbu>,
                                       CbuRepositoryCustom {
    
    // Simple queries - Spring generates implementation
    Optional<Cbu> findByName(String name);
    List<Cbu> findByJurisdiction(String jurisdiction);
    List<Cbu> findByStatus(CbuStatus status);
    boolean existsByName(String name);
    
    // But you still need JPQL for anything non-trivial
    @Query("SELECT c FROM Cbu c WHERE c.jurisdiction = :jurisdiction AND c.status = :status")
    List<Cbu> findByJurisdictionAndStatus(
        @Param("jurisdiction") String jurisdiction, 
        @Param("status") CbuStatus status
    );
    
    // And explicit fetching to avoid N+1
    @EntityGraph(attributePaths = {"entityRoles", "productSubscriptions"})
    Optional<Cbu> findWithAssociationsById(UUID id);
    
    @Query("SELECT DISTINCT c FROM Cbu c " +
           "LEFT JOIN FETCH c.entityRoles er " +
           "LEFT JOIN FETCH er.entity " +
           "WHERE c.cbuId = :id")
    Optional<Cbu> findWithEntityRolesAndEntitiesById(@Param("id") UUID id);
    
    // Modifying queries need explicit annotation
    @Modifying
    @Query("UPDATE Cbu c SET c.status = :status, c.updatedAt = CURRENT_TIMESTAMP WHERE c.cbuId = :id")
    int updateStatus(@Param("id") UUID id, @Param("status") CbuStatus status);
}

// Custom implementation for complex queries - STILL NEEDED
public interface CbuRepositoryCustom {
    Page<CbuSummaryProjection> searchWithFilters(CbuSearchCriteria criteria, Pageable pageable);
    List<CbuWithStatsProjection> findAllWithAggregatedStats();
}

@Repository
@RequiredArgsConstructor
public class CbuRepositoryCustomImpl implements CbuRepositoryCustom {
    
    private final EntityManager entityManager;
    
    @Override
    public Page<CbuSummaryProjection> searchWithFilters(CbuSearchCriteria criteria, Pageable pageable) {
        var cb = entityManager.getCriteriaBuilder();
        var query = cb.createQuery(CbuSummaryProjection.class);
        var root = query.from(Cbu.class);
        
        var predicates = new ArrayList<Predicate>();
        
        if (StringUtils.hasText(criteria.getName())) {
            predicates.add(cb.like(
                cb.lower(root.get("name")), 
                "%" + criteria.getName().toLowerCase() + "%"
            ));
        }
        
        if (criteria.getJurisdiction() != null) {
            predicates.add(cb.equal(root.get("jurisdiction"), criteria.getJurisdiction()));
        }
        
        if (criteria.getStatuses() != null && !criteria.getStatuses().isEmpty()) {
            predicates.add(root.get("status").in(criteria.getStatuses()));
        }
        
        if (criteria.getCreatedAfter() != null) {
            predicates.add(cb.greaterThanOrEqualTo(root.get("createdAt"), criteria.getCreatedAfter()));
        }
        
        query.where(predicates.toArray(new Predicate[0]));
        
        // Count query for pagination
        var countQuery = cb.createQuery(Long.class);
        var countRoot = countQuery.from(Cbu.class);
        countQuery.select(cb.count(countRoot));
        countQuery.where(predicates.toArray(new Predicate[0]));
        var total = entityManager.createQuery(countQuery).getSingleResult();
        
        // Main query with pagination
        var typedQuery = entityManager.createQuery(query);
        typedQuery.setFirstResult((int) pageable.getOffset());
        typedQuery.setMaxResults(pageable.getPageSize());
        
        var results = typedQuery.getResultList();
        
        return new PageImpl<>(results, pageable, total);
    }
    
    // ... another 100+ lines for other custom queries
}
```

**Even with Spring Data's magic, you still need:**
- Custom repository interfaces for anything complex
- JPQL/Criteria API implementations  
- Explicit `@EntityGraph` or `JOIN FETCH` for every fetch pattern
- `@Modifying` annotation decisions
- Pagination boilerplate

**90 repositories × 80 lines average = 7,200+ lines**

### The ob-poc Equivalent

```yaml
# All query patterns defined declaratively
read:
  behavior: crud
  crud:
    operation: select
    table: cbus
  args:
    - name: cbu-id
      type: uuid
      required: true

list:
  behavior: crud
  crud:
    operation: select
    table: cbus
    multiple: true
  args:
    - name: jurisdiction
      type: string
      required: false
    - name: status
      type: string
      required: false
    - name: limit
      type: integer
      required: false
      default: 100
```

**One generic executor. All entities. All query patterns.**

---

## 3. The Service Layer (Modern Patterns)

### Current Best Practice Service

```java
@Service
@RequiredArgsConstructor  // Constructor injection via Lombok
@Slf4j
@Transactional(readOnly = true)  // Read-only by default, override for writes
public class CbuService {
    
    private final CbuRepository cbuRepository;
    private final EntityRepository entityRepository;
    private final ProductService productService;
    private final EventPublisher eventPublisher;
    private final CbuMapper cbuMapper;
    private final Validator validator;
    
    @Transactional
    public CbuResponse createCbu(CreateCbuRequest request) {
        log.info("Creating CBU: {}", request.name());
        
        // Validate (Bean Validation + custom)
        validateCreateRequest(request);
        
        // Check for duplicates
        if (cbuRepository.existsByName(request.name())) {
            throw new DuplicateCbuException("CBU already exists: " + request.name());
        }
        
        // Build entity
        var cbu = Cbu.builder()
            .name(request.name())
            .jurisdiction(request.jurisdiction())
            .clientType(request.clientType())
            .status(CbuStatus.PROSPECT)
            .build();
        
        // Save
        var saved = cbuRepository.save(cbu);
        
        // Publish domain event
        eventPublisher.publishEvent(new CbuCreatedEvent(saved.getCbuId(), saved.getName()));
        
        log.info("Created CBU with ID: {}", saved.getCbuId());
        
        return cbuMapper.toResponse(saved);
    }
    
    @Transactional
    public CbuResponse updateCbu(UUID cbuId, UpdateCbuRequest request) {
        log.info("Updating CBU: {}", cbuId);
        
        var cbu = cbuRepository.findById(cbuId)
            .orElseThrow(() -> new CbuNotFoundException(cbuId));
        
        // Validate state transitions
        if (request.status() != null && request.status() != cbu.getStatus()) {
            validateStatusTransition(cbu.getStatus(), request.status());
        }
        
        // Update fields (null-safe)
        Optional.ofNullable(request.name()).ifPresent(cbu::setName);
        Optional.ofNullable(request.status()).ifPresent(cbu::setStatus);
        
        var saved = cbuRepository.save(cbu);
        
        eventPublisher.publishEvent(new CbuUpdatedEvent(saved.getCbuId()));
        
        return cbuMapper.toResponse(saved);
    }
    
    public CbuResponse getCbu(UUID cbuId) {
        return cbuRepository.findById(cbuId)
            .map(cbuMapper::toResponse)
            .orElseThrow(() -> new CbuNotFoundException(cbuId));
    }
    
    public CbuDetailResponse getCbuWithDetails(UUID cbuId) {
        var cbu = cbuRepository.findWithAssociationsById(cbuId)
            .orElseThrow(() -> new CbuNotFoundException(cbuId));
        return cbuMapper.toDetailResponse(cbu);
    }
    
    public Page<CbuSummaryResponse> searchCbus(CbuSearchCriteria criteria, Pageable pageable) {
        return cbuRepository.searchWithFilters(criteria, pageable)
            .map(cbuMapper::toSummaryResponse);
    }
    
    @Transactional
    public void addProduct(UUID cbuId, AddProductRequest request) {
        var cbu = cbuRepository.findById(cbuId)
            .orElseThrow(() -> new CbuNotFoundException(cbuId));
        
        // Business rule validation
        if (cbu.getStatus() == CbuStatus.CLOSED) {
            throw new InvalidOperationException("Cannot add product to closed CBU");
        }
        
        // Check for duplicate subscription
        var alreadySubscribed = cbu.getProductSubscriptions().stream()
            .anyMatch(s -> s.getProduct().getCode().equals(request.productCode()));
        
        if (alreadySubscribed) {
            throw new DuplicateSubscriptionException(
                "CBU already subscribed to: " + request.productCode()
            );
        }
        
        // Delegate to product service
        productService.subscribeToProduct(cbu, request.productCode());
        
        eventPublisher.publishEvent(
            new ProductSubscribedEvent(cbuId, request.productCode())
        );
    }
    
    private void validateCreateRequest(CreateCbuRequest request) {
        var violations = validator.validate(request);
        if (!violations.isEmpty()) {
            throw new ValidationException(violations);
        }
        // Additional custom validation...
    }
    
    private void validateStatusTransition(CbuStatus from, CbuStatus to) {
        var validTransitions = Map.of(
            CbuStatus.PROSPECT, Set.of(CbuStatus.ACTIVE, CbuStatus.CLOSED),
            CbuStatus.ACTIVE, Set.of(CbuStatus.DORMANT, CbuStatus.CLOSED),
            CbuStatus.DORMANT, Set.of(CbuStatus.ACTIVE, CbuStatus.CLOSED),
            CbuStatus.CLOSED, Set.of()  // Terminal state
        );
        
        if (!validTransitions.getOrDefault(from, Set.of()).contains(to)) {
            throw new InvalidStateTransitionException(from, to);
        }
    }
}
```

**This is genuinely modern Spring code.** Constructor injection, records for requests, 
Lombok, proper transaction boundaries, event publishing.

**It's still 150+ lines for basic CRUD on one entity.**

And you need similar services for:
- EntityService
- KycCaseService  
- WorkstreamService
- ProductService
- SsiService
- TradingProfileService
- ... (60+ services)

**60 services × 200 lines = 12,000+ lines of service code**

### The ob-poc Equivalent

```rust
// One executor handles ALL CRUD operations
impl GenericCrudExecutor {
    pub async fn execute(&self, verb: &VerbDef, args: &Args, pool: &PgPool) -> Result<Value> {
        let sql = self.build_sql(verb, args)?;
        let result = sqlx::query(&sql).fetch_one(pool).await?;
        Ok(row_to_json(result))
    }
}
```

**~500 lines handles what Spring needs 12,000+ lines for.**

---

## 4. DTOs With Java Records (Modern Approach)

### Even With Records, You Need Many

```java
// Request DTOs
public record CreateCbuRequest(
    @NotBlank @Size(max = 255) String name,
    @NotBlank @Pattern(regexp = "^[A-Z]{2}$") String jurisdiction,
    @NotNull ClientType clientType
) {}

public record UpdateCbuRequest(
    @Size(max = 255) String name,
    CbuStatus status
) {}

public record AddProductRequest(
    @NotBlank String productCode
) {}

public record CbuSearchCriteria(
    String name,
    String jurisdiction,
    Set<CbuStatus> statuses,
    Instant createdAfter,
    Instant createdBefore
) {}

// Response DTOs  
public record CbuResponse(
    UUID cbuId,
    String name,
    String jurisdiction,
    String clientType,
    String status,
    Instant createdAt
) {}

public record CbuDetailResponse(
    UUID cbuId,
    String name,
    String jurisdiction,
    String clientType,
    String status,
    Instant createdAt,
    List<EntityRoleSummary> entityRoles,
    List<ProductSubscriptionSummary> products,
    List<KycCaseSummary> kycCases
) {}

public record CbuSummaryResponse(
    UUID cbuId,
    String name,
    String jurisdiction,
    String status,
    int entityCount,
    int productCount
) {}

// Nested DTOs
public record EntityRoleSummary(UUID entityId, String entityName, String roleType) {}
public record ProductSubscriptionSummary(String productCode, String status, LocalDate effectiveDate) {}
public record KycCaseSummary(UUID caseId, String caseType, String status) {}
```

**That's 7 DTOs for ONE entity.** And you need projections/views:

```java
// Projections for optimized queries
public interface CbuSummaryProjection {
    UUID getCbuId();
    String getName();
    String getJurisdiction();
    String getStatus();
}

public interface CbuWithStatsProjection {
    UUID getCbuId();
    String getName();
    Long getEntityCount();
    Long getProductCount();
}
```

**90 entities × 5-7 DTOs each = 450-630 DTO classes**

Records are cleaner than old-style classes, but you still need ALL OF THEM.

### The ob-poc Equivalent

```lisp
(cbu.read cbu-id:@alpha-fund)
```

Returns exactly what the verb definition says. No DTO classes.

```yaml
# Control response shape in config
read:
  returns:
    - cbu_id
    - name
    - jurisdiction
    - status
    - created_at
```

---

## 5. Mappers With MapStruct (Best Practice)

### Even Compile-Time Generated Mappers Need Definitions

```java
@Mapper(
    componentModel = "spring",
    injectionStrategy = InjectionStrategy.CONSTRUCTOR,
    unmappedTargetPolicy = ReportingPolicy.ERROR,
    uses = {EntityMapper.class, ProductMapper.class, KycMapper.class}
)
public interface CbuMapper {
    
    CbuResponse toResponse(Cbu entity);
    
    CbuDetailResponse toDetailResponse(Cbu entity);
    
    CbuSummaryResponse toSummaryResponse(CbuSummaryProjection projection);
    
    @Mapping(target = "cbuId", ignore = true)
    @Mapping(target = "createdAt", ignore = true)
    @Mapping(target = "createdBy", ignore = true)
    @Mapping(target = "updatedAt", ignore = true)
    @Mapping(target = "updatedBy", ignore = true)
    @Mapping(target = "version", ignore = true)
    @Mapping(target = "entityRoles", ignore = true)
    @Mapping(target = "productSubscriptions", ignore = true)
    @Mapping(target = "kycCases", ignore = true)
    @Mapping(target = "ssis", ignore = true)
    @Mapping(target = "instrumentUniverse", ignore = true)
    @Mapping(target = "status", constant = "PROSPECT")
    Cbu toEntity(CreateCbuRequest request);
    
    @BeanMapping(nullValuePropertyMappingStrategy = NullValuePropertyMappingStrategy.IGNORE)
    @Mapping(target = "cbuId", ignore = true)
    @Mapping(target = "createdAt", ignore = true)
    @Mapping(target = "createdBy", ignore = true)
    @Mapping(target = "entityRoles", ignore = true)
    @Mapping(target = "productSubscriptions", ignore = true)
    @Mapping(target = "kycCases", ignore = true)
    @Mapping(target = "ssis", ignore = true)
    @Mapping(target = "instrumentUniverse", ignore = true)
    void updateFromRequest(UpdateCbuRequest request, @MappingTarget Cbu entity);
    
    // Nested mappings
    default List<EntityRoleSummary> mapEntityRoles(Set<CbuEntityRole> roles) {
        if (roles == null) return List.of();
        return roles.stream()
            .map(r -> new EntityRoleSummary(
                r.getEntity().getEntityId(),
                r.getEntity().getName(),
                r.getRoleType().name()
            ))
            .toList();
    }
}
```

**MapStruct is great.** It generates efficient code at compile time.

**But you still need to define every mapping.** And those `@Mapping(target = "...", ignore = true)` 
annotations? Miss one and you get a compile error—or worse, a silent bug.

**90 mappers × 50 lines = 4,500 lines of mapping definitions**

### The ob-poc Equivalent

No mappers. The executor returns JSON directly from the query result.

---

## 6. The N+1 Problem Is Still Real

### This Is Not Solved By Modern Spring

```java
// This code STILL causes N+1 in 2024
@Transactional(readOnly = true)
public List<CbuSummaryResponse> getAllCbus() {
    return cbuRepository.findAll().stream()  // 1 query
        .map(cbu -> new CbuSummaryResponse(
            cbu.getCbuId(),
            cbu.getName(),
            cbu.getJurisdiction(),
            cbu.getStatus().name(),
            cbu.getEntityRoles().size(),     // N queries!
            cbu.getProductSubscriptions().size()  // N more queries!
        ))
        .toList();
}
```

**The "solutions" haven't changed:**

```java
// Option 1: EntityGraph (must define for each use case)
@EntityGraph(attributePaths = {"entityRoles", "productSubscriptions"})
List<Cbu> findAllWithAssociations();

// Option 2: JOIN FETCH (Cartesian product risk)
@Query("SELECT DISTINCT c FROM Cbu c " +
       "LEFT JOIN FETCH c.entityRoles " +
       "LEFT JOIN FETCH c.productSubscriptions")
List<Cbu> findAllWithFetch();

// Option 3: Batch size (still N/batch queries)
@BatchSize(size = 50)
private Set<CbuEntityRole> entityRoles;

// Option 4: DTO projection (write SQL anyway)
@Query("""
    SELECT new com.example.CbuSummaryResponse(
        c.cbuId, c.name, c.jurisdiction, c.status,
        (SELECT COUNT(r) FROM CbuEntityRole r WHERE r.cbu = c),
        (SELECT COUNT(p) FROM CbuProductSubscription p WHERE p.cbu = c)
    )
    FROM Cbu c
    """)
List<CbuSummaryResponse> findAllSummaries();
```

Every approach has tradeoffs. You end up with 5+ query methods per repository
for different fetch patterns.

### The ob-poc Approach

```sql
-- Write the query you need
SELECT 
    c.cbu_id,
    c.name,
    COUNT(DISTINCT r.role_id) as entity_count,
    COUNT(DISTINCT p.subscription_id) as product_count
FROM cbus c
LEFT JOIN cbu_entity_roles r ON c.cbu_id = r.cbu_id
LEFT JOIN cbu_product_subscriptions p ON c.cbu_id = p.cbu_id
GROUP BY c.cbu_id;
```

**One query. Exactly what you need. No ORM interpretation.**

---

## 7. Testing Reality

### Modern Spring Testing (Still Slow)

```java
@SpringBootTest
@Testcontainers
@AutoConfigureTestDatabase(replace = Replace.NONE)
class CbuServiceIntegrationTest {
    
    @Container
    @ServiceConnection
    static PostgreSQLContainer<?> postgres = new PostgreSQLContainer<>("postgres:15-alpine");
    
    @Autowired
    CbuService cbuService;
    
    @Autowired
    CbuRepository cbuRepository;
    
    @Test
    void createCbu_shouldCreateAndReturnCbu() {
        // Arrange
        var request = new CreateCbuRequest("Test Fund", "US", ClientType.FUND);
        
        // Act
        var result = cbuService.createCbu(request);
        
        // Assert
        assertThat(result.cbuId()).isNotNull();
        assertThat(result.name()).isEqualTo("Test Fund");
        
        // Verify persistence
        var saved = cbuRepository.findById(result.cbuId());
        assertThat(saved).isPresent();
    }
}
```

**Test startup with `@SpringBootTest` + Testcontainers:**
- First test: 15-30 seconds (container startup + Spring context)
- Subsequent tests: 1-5 seconds each (if context reused)
- Context reload: +15-30 seconds

**500 integration tests = 30-60 minutes** (with context caching)

**Common pain points:**
- `@DirtiesContext` forces reload
- Different `@MockBean` combinations break caching
- Testcontainers startup per module
- Memory pressure from multiple contexts

### The ob-poc Approach

```rust
#[tokio::test]
async fn test_cbu_create() {
    let pool = test_pool().await;  // Shared, <100ms setup
    let mut ctx = ExecutionContext::new();
    
    let result = execute_dsl(
        &pool,
        r#"(cbu.create name:"Test Fund" jurisdiction:US)"#,
        &mut ctx
    ).await.unwrap();
    
    assert!(result["cbu_id"].is_string());
}
```

**No framework. No context. No magic. <1 second per test.**

---

## 8. The Configuration Sprawl

### Modern Spring Boot (Still Complex)

```yaml
# application.yml
spring:
  application:
    name: onboarding-service
  datasource:
    url: jdbc:postgresql://localhost:5432/onboarding
    username: ${DB_USERNAME}
    password: ${DB_PASSWORD}
    hikari:
      maximum-pool-size: 20
      minimum-idle: 5
      connection-timeout: 30000
  jpa:
    hibernate:
      ddl-auto: validate
    properties:
      hibernate:
        format_sql: true
        default_batch_fetch_size: 20
        jdbc:
          batch_size: 50
        order_inserts: true
        order_updates: true
    open-in-view: false  # Critical!
  flyway:
    enabled: true
    locations: classpath:db/migration
  cache:
    type: caffeine
    caffeine:
      spec: maximumSize=1000,expireAfterWrite=5m

logging:
  level:
    org.hibernate.SQL: DEBUG
    org.hibernate.type.descriptor.sql: TRACE
    
# Plus application-dev.yml, application-prod.yml, etc.
```

Plus Java configuration:

```java
@Configuration
@EnableJpaAuditing
public class JpaConfig {
    
    @Bean
    public AuditorAware<UUID> auditorProvider() {
        return () -> Optional.ofNullable(SecurityContextHolder.getContext())
            .map(SecurityContext::getAuthentication)
            .filter(Authentication::isAuthenticated)
            .map(auth -> UUID.fromString(auth.getName()));
    }
}

@Configuration
@EnableCaching
public class CacheConfig {
    
    @Bean
    public CacheManager cacheManager() {
        var caffeine = Caffeine.newBuilder()
            .maximumSize(1000)
            .expireAfterWrite(Duration.ofMinutes(5))
            .recordStats();
        return new CaffeineCacheManager("cbus", "entities", "products");
    }
}

@Configuration
public class AsyncConfig implements AsyncConfigurer {
    
    @Override
    public Executor getAsyncExecutor() {
        var executor = new ThreadPoolTaskExecutor();
        executor.setCorePoolSize(5);
        executor.setMaxPoolSize(10);
        executor.setQueueCapacity(100);
        executor.initialize();
        return executor;
    }
}

// ... more @Configuration classes
```

**Miss one configuration option?**
- `open-in-view: false` not set → LazyInitializationException in production
- `ddl-auto` wrong → database corruption
- Pool size wrong → connection exhaustion under load
- Batch settings wrong → N+1 still happens

### The ob-poc Approach

```yaml
# config/database.yaml
host: localhost
port: 5432
database: ob_poc
pool_size: 20

# That's it. Business config is in verb definitions.
```

---

## 9. The Real Comparison

### Spring/JPA Project Scope (Honest Estimate)

| Component | Files | Lines (with Lombok/Records) |
|-----------|-------|----------------------------|
| Entity classes | 90 | 9,000 |
| Repository interfaces | 90 | 4,500 |
| Custom repository impls | 30 | 3,000 |
| Service classes | 60 | 12,000 |
| DTOs (Records) | 400 | 8,000 |
| Mappers (MapStruct) | 90 | 4,500 |
| Controllers | 40 | 4,000 |
| Configuration | 20 | 1,500 |
| Exception handling | 30 | 1,500 |
| Event classes | 50 | 1,000 |
| **Subtotal (Java)** | **900** | **49,000** |
| Tests | 600 | 30,000 |
| **Total** | **1,500** | **79,000** |

This is WITH Lombok, Records, MapStruct, Spring Data magic.

**Without those tools: 120,000+ lines.**

### ob-poc Actual

| Component | Files | Lines |
|-----------|-------|-------|
| DSL parser | 5 | 2,000 |
| Verb registry | 3 | 1,500 |
| Generic executor | 5 | 3,000 |
| Custom operations | 15 | 5,000 |
| API layer | 10 | 4,000 |
| YAML verb configs | 70 | 5,000 |
| Schema migrations | 30 | 3,000 |
| Tests | 20 | 5,000 |
| **Total** | **158** | **28,500** |

---

## 10. The Staffing Math

### Spring Project Team

| Role | Months | Why |
|------|--------|-----|
| 2 Senior Java Devs | 12 | Entity + Service layer |
| 2 Mid Java Devs | 12 | DTOs, mappers, tests |
| 1 JPA Expert | 6 | N+1 hunting, performance |
| 1 DevOps | 6 | Build, deploy, monitoring |
| 1 QA | 12 | Integration testing |
| 1 Tech Lead | 12 | Architecture, reviews |

**Total: 8 people × 12 months average = 96 person-months**

### ob-poc Team

| Role | Months | Why |
|------|--------|-----|
| 2-3 Rust/Go Devs | 3 | Core platform is built |
| 1 Domain Expert | 3 | Verb definitions |
| 1 QA | 2 | Integration tests |

**Total: 4 people × 3 months = 12 person-months**

**Ratio: 8:1**

---

## 11. What About Spring Data REST?

### The "Zero Code" Promise

```java
@RepositoryRestResource(path = "cbus")
public interface CbuRepository extends JpaRepository<Cbu, UUID> {
}
// "That's it! REST API generated!"
```

### The Reality

1. **Still need entity classes** with all annotations
2. **Exposes internal model** - bad API design
3. **No business logic** - just CRUD
4. **Custom endpoints anyway** for real operations
5. **N+1 still happens** in projections
6. **Security is tricky** - which fields to expose?
7. **No audit trail** without custom events

You end up writing custom controllers anyway, plus fighting the framework.

---

## 12. The AI Factor

### Why AI Struggles With Spring

Ask Claude/GPT to generate a complete Spring service:

```java
// AI output often has issues:
@Service
public class CbuService {
    @Autowired  // Field injection - now discouraged
    private CbuRepository repo;
    
    public Cbu create(CreateCbuRequest request) {
        // Missing @Transactional
        // Missing validation
        // Missing duplicate check
        // Missing event publishing
        // Missing audit
        // Wrong return type (entity not DTO)
        Cbu cbu = new Cbu();
        cbu.setName(request.getName());
        return repo.save(cbu);
    }
}
```

**Problems AI commonly gets wrong:**
- Annotation placement and options
- Transaction boundaries
- Which of 20 Spring patterns to use
- Correct dependency injection style
- Exception handling patterns
- Cross-cutting concerns

### Why AI Works With DSL

```lisp
(cbu.create name:"Alpha Fund" jurisdiction:US)
```

**Why this works:**
1. **Constrained vocabulary** - Only valid verbs exist
2. **Parser validates** - Syntax errors caught immediately
3. **No hidden concerns** - What you see is what runs
4. **Composable** - Each line is independent
5. **Self-documenting** - Verb names ARE the API

---

## Summary: The Honest Trade-off

### Spring/JPA Gives You

✅ Industry familiarity
✅ Abundant documentation  
✅ IDE support
✅ Hiring pool
✅ Vendor support options

### Spring/JPA Costs You

❌ 79,000+ lines of code
❌ 15-45 second test startup
❌ N+1 query whack-a-mole
❌ Annotation configuration complexity
❌ 12+ month timeline
❌ 8+ person team
❌ Cannot leverage AI generation

### ob-poc Gives You

✅ 28,500 lines of code
✅ <1 second test runs
✅ Explicit SQL, predictable performance
✅ Configuration-driven changes
✅ 3 month timeline
✅ 4-5 person team
✅ AI can generate and extend

### ob-poc Costs You

❌ Less industry familiarity
❌ Smaller hiring pool
❌ Custom documentation needed
❌ No vendor support

---

## The Question

> "We could build this in Spring. Our team knows it."

The response:

> "Yes, and it would take 12 months, 8 people, and 79,000 lines of code.
> The system running today is 28,500 lines. 
> Which approach delivers more value faster?"
