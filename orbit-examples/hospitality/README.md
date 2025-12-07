# Coffeehouse & Restaurant Operations - OrbitRS Examples

## Overview

Comprehensive hospitality industry examples demonstrating OrbitRS's multi-protocol capabilities for coffeehouse chains, quick-service restaurants, and fine dining establishments.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│           Hospitality Operations Platform on OrbitRS                │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────┐ │
│  │  PostgreSQL  │  │    Redis     │  │   MongoDB    │  │ Neo4j  │ │
│  │   :5432      │  │    :6379     │  │   :27017     │  │ :7687  │ │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  ├────────┤ │
│  │ Menu Items   │  │ Order Queue  │  │ Recipes      │  │ Recom. │ │
│  │ POS Orders   │  │ Mobile Orders│  │ Menu Images  │  │ Prefs  │ │
│  │ Stores       │  │ Loyalty Pts  │  │ Preferences  │  │ Supply │ │
│  │ Customers    │  │ Wait Times   │  │ Campaigns    │  │ Chain  │ │
│  │ Inventory    │  │ Real-time    │  │ Delivery     │  │        │ │
│  │ Staff        │  │ Stock        │  │ Integration  │  │        │ │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────┘ │
│                                                                      │
│  ┌──────────────┐  ┌──────────────────────────────────────────┐   │
│  │  Cassandra   │  │      Workflows & Integration             │   │
│  │   :9042      │  ├──────────────────────────────────────────┤   │
│  ├──────────────┤  │ • Mobile Order Processing                │   │
│  │ Sales Trends │  │ • Loyalty Rewards Engine                 │   │
│  │ Peak Hours   │  │ • Kitchen Display System                 │   │
│  │ Visit        │  │ • Inventory Replenishment                │   │
│  │ Patterns     │  │ • Delivery Integration                   │   │
│  │ Labor Costs  │  │ • Analytics Dashboard                    │   │
│  └──────────────┘  └──────────────────────────────────────────┘   │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Use Cases

### 1. Mobile Order Ahead
**Scenario**: Customer orders via mobile app for pickup

**Protocols**:
- **Redis**: Real-time order queue, estimated wait time
- **PostgreSQL**: Order record, payment processing
- **MongoDB**: Customer preferences, favorite orders
- **Neo4j**: Personalized recommendations

**Performance**: <2 seconds order placement

### 2. Loyalty Rewards
**Scenario**: Earn and redeem points with every purchase

**Protocols**:
- **PostgreSQL**: Customer account, transaction history
- **Redis**: Real-time points balance, tier status
- **Cassandra**: Points history, redemption analytics

### 3. Kitchen Operations
**Scenario**: Real-time order routing to kitchen stations

**Protocols**:
- **Redis**: Order queue by station (espresso, food, pastry)
- **MongoDB**: Recipe instructions, preparation guides
- **Cassandra**: Preparation time analytics

### 4. Inventory Management
**Scenario**: Track ingredients, predict shortages

**Protocols**:
- **PostgreSQL**: Inventory levels, supplier orders
- **Redis**: Real-time stock alerts
- **Cassandra**: Usage patterns, waste tracking

## Key Features

- **Multi-Location**: Chain operations with centralized management
- **Mobile Ordering**: Order ahead, curbside pickup
- **Loyalty Programs**: Points, tiers, personalized offers
- **Kitchen Display**: Real-time order routing
- **Inventory Tracking**: Ingredient-level tracking
- **Analytics**: Sales trends, popular items, peak hours
- **Delivery Integration**: Third-party delivery platforms
- **Staff Management**: Scheduling, labor cost tracking

## Quick Start

```bash
# Set up schemas
psql -h localhost -p 5432 -U orbit -d hospitality < sql/01_schema_menu.sql
psql -h localhost -p 5432 -U orbit -d hospitality < sql/02_schema_pos.sql

# Initialize Redis
redis-cli -h localhost -p 6379 < redis/01_operations.redis

# Set up MongoDB
mongosh mongodb://localhost:27017/hospitality mongodb/01_menu_catalog.js

# Run mobile order workflow
cd python
python3 01_mobile_order.py --customer-id cust-001
```

## Directory Structure

```
hospitality/
├── sql/                    # PostgreSQL schemas
├── redis/                  # Redis operations
├── mongodb/                # MongoDB collections
├── cypher/                 # Neo4j graphs
├── cql/                    # Cassandra tables
├── python/                 # Python workflows
├── javascript/             # JavaScript integration
└── workflows/              # Documentation
```

## Performance Benchmarks

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Mobile Order | <2s | 10K/sec |
| POS Transaction | <500ms | 20K/sec |
| Loyalty Points Update | <100ms | 50K/sec |
| Menu Lookup | <50ms | 100K/sec |
| Order Queue Update | <10ms | 100K/sec |

## Business Models Covered

- **Coffeehouse Chain**: Espresso bar, specialty drinks, mobile ordering
- **Quick-Service Restaurant**: Breakfast/lunch, fast casual
- **French Bistro**: Fine dining, table service, reservations
- **Bakery/Café**: Pastries, sandwiches, catering
