# Travel Booking Platform - OrbitRS Examples

## Overview

Comprehensive travel booking platform examples demonstrating OrbitRS's multi-protocol capabilities for flight booking, hotel reservations, car rentals, and vacation packages - similar to Expedia, Orbitz, and Travelocity.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│           Travel Booking Platform on OrbitRS                        │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────┐   │
│  │  PostgreSQL  │  │    Redis     │  │   MongoDB    │  │ Neo4j  │   │
│  │   :5432      │  │    :6379     │  │   :27017     │  │ :7687  │   │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  ├────────┤   │
│  │ Flights      │  │ Search Cache │  │ Packages     │  │ Recom. │   │
│  │ Hotels       │  │ Pricing      │  │ Preferences  │  │ Routes │   │
│  │ Cars         │  │ Availability │  │ Reviews      │  │ Travel │   │
│  │ Bookings     │  │ Loyalty Pts  │  │ Destinations │  │ Graph  │   │
│  │ Customers    │  │ Sessions     │  │              │  │        │   │
│  │ Payments     │  │ Real-time    │  │              │  │        │   │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────┘   │
│                                                                     │
│  ┌──────────────┐  ┌──────────────────────────────────────────┐     │
│  │  Cassandra   │  │      Workflows & Integration             │     │
│  │   :9042      │  ├──────────────────────────────────────────┤     │
│  ├──────────────┤  │ • Flight Search & Booking                │     │
│  │ Analytics    │  │ • Hotel Reservations                     │     │
│  │ Booking Hist │  │ • Vacation Packages                      │     │
│  │ Trends       │  │ • Multi-City Itineraries                 │     │
│  │ Revenue      │  │ • Loyalty Rewards                        │     │
│  └──────────────┘  └──────────────────────────────────────────┘     │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

## Use Cases

### 1. Flight Search & Booking
**Scenario**: Customer searches and books a flight

**Protocols**:
- **Redis**: Search result caching, real-time pricing, seat availability
- **PostgreSQL**: Flight schedules, inventory, bookings, payments
- **MongoDB**: Customer preferences, search history
- **Neo4j**: Route recommendations, travel patterns

**Performance**: <2 seconds search, <500ms booking

### 2. Hotel Reservations
**Scenario**: Book hotel with personalized recommendations

**Protocols**:
- **PostgreSQL**: Hotel inventory, room types, pricing, bookings
- **Redis**: Availability cache, dynamic pricing
- **MongoDB**: Hotel reviews, ratings, amenities
- **Neo4j**: Similar hotels, destination recommendations

### 3. Vacation Packages
**Scenario**: Book bundled flight + hotel + car with discount

**Protocols**:
- **MongoDB**: Package catalog, flexible schema for deals
- **PostgreSQL**: Component bookings, pricing
- **Redis**: Bundle discounts, flash sales
- **Neo4j**: Package recommendations based on preferences

### 4. Loyalty Rewards
**Scenario**: Earn and redeem points with every booking

**Protocols**:
- **Redis**: Real-time points balance, tier status
- **PostgreSQL**: Loyalty accounts, transaction history
- **Neo4j**: Referral networks, partner programs

## Key Features

- **Multi-Protocol Search**: Flight, hotel, and car search with intelligent caching
- **Dynamic Pricing**: Real-time fare updates, surge pricing, discounts
- **Loyalty Program**: Points earning, tier benefits, redemption
- **Personalization**: Preferences, recommendations, saved searches
- **Package Deals**: Bundled bookings with discounts
- **Real-time Inventory**: Seat and room availability tracking
- **Price Alerts**: Notifications for price drops
- **Multi-City Trips**: Complex itinerary planning

## Quick Start

```bash
# Set up PostgreSQL schemas
psql -h localhost -p 5432 -U orbit -d travel < sql/01_schema_flights.sql
psql -h localhost -p 5432 -U orbit -d travel < sql/02_schema_hotels.sql
psql -h localhost -p 5432 -U orbit -d travel < sql/03_schema_cars.sql
psql -h localhost -p 5432 -U orbit -d travel < sql/04_schema_packages.sql

# Initialize Redis
redis-cli -h localhost -p 6379 < redis/01_search_cache.redis
redis-cli -h localhost -p 6379 < redis/02_pricing_engine.redis
redis-cli -h localhost -p 6379 < redis/03_loyalty_points.redis

# Set up MongoDB
mongosh mongodb://localhost:27017/travel --file mongodb/01_travel_packages.js

# Run flight booking workflow
cd python
python3 01_flight_booking.py
```

## Directory Structure

```
travel/
├── sql/                    # PostgreSQL schemas
│   ├── 01_schema_flights.sql
│   ├── 02_schema_hotels.sql
│   ├── 03_schema_cars.sql
│   └── 04_schema_packages.sql
├── redis/                  # Redis operations
│   ├── 01_search_cache.redis
│   ├── 02_pricing_engine.redis
│   └── 03_loyalty_points.redis
├── mongodb/                # MongoDB collections
│   ├── 01_travel_packages.js
│   ├── 02_user_preferences.js
│   └── 03_reviews_ratings.js
├── cypher/                 # Neo4j graphs
│   ├── 01_recommendations.cypher
│   └── 02_loyalty_network.cypher
├── cql/                    # Cassandra tables
│   └── 01_booking_analytics.cql
├── python/                 # Python workflows
│   ├── 01_flight_booking.py
│   ├── 02_hotel_reservation.py
│   ├── 03_package_booking.py
│   └── 04_multi_city_itinerary.py
├── javascript/             # JavaScript integration
│   └── 01_search_integration.js
└── workflows/              # Documentation
    └── 01_flight_booking_workflow.md
```

## Performance Benchmarks

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Flight Search | <2s | 5K/sec |
| Hotel Search | <1.5s | 6K/sec |
| Booking Creation | <500ms | 10K/sec |
| Loyalty Points Update | <100ms | 50K/sec |
| Price Cache Lookup | <10ms | 100K/sec |
| Availability Check | <50ms | 20K/sec |

## Data Models

### Flights
- **Airlines**: Carriers, alliances, metadata
- **Airports**: Locations, timezones, facilities
- **Flights**: Schedules, routes, aircraft
- **Inventory**: Daily seat availability by class
- **Pricing**: Dynamic fares, fare classes, restrictions
- **Bookings**: Reservations, passengers, payments

### Hotels
- **Chains**: Hotel brands, loyalty programs
- **Properties**: Locations, amenities, ratings
- **Rooms**: Types, features, capacity
- **Inventory**: Daily room availability
- **Pricing**: Dynamic rates, seasonal pricing
- **Bookings**: Reservations, special requests

### Cars
- **Companies**: Rental agencies, locations
- **Vehicles**: Categories, features, capacity
- **Inventory**: Daily vehicle availability
- **Pricing**: Rates, insurance, add-ons
- **Bookings**: Rentals, mileage, fuel

### Packages
- **Customers**: Profiles, preferences, documents
- **Packages**: Bundled deals, destinations
- **Bookings**: Multi-component reservations
- **Loyalty**: Points, tiers, rewards

## Example Workflows

### Flight Booking (Python)
```python
from flight_booking import FlightBookingWorkflow

workflow = FlightBookingWorkflow()
result = workflow.search_and_book_flight(
    customer_id='cust-001',
    origin='SFO',
    destination='JFK',
    departure_date='2024-12-15',
    passengers=2,
    cabin_class='ECONOMY'
)
```

**Workflow Steps**:
1. Check search cache (Redis)
2. Search flights (PostgreSQL)
3. Get customer preferences (MongoDB)
4. Calculate pricing with loyalty discount (Redis + PostgreSQL)
5. Create booking (PostgreSQL)
6. Update inventory (PostgreSQL + Redis)
7. Award loyalty points (Redis)
8. Store preferences (MongoDB)
9. Update recommendations (Neo4j)
10. Send confirmation (Redis pub/sub)

## Business Models Covered

- **Online Travel Agency (OTA)**: Expedia, Orbitz, Travelocity model
- **Metasearch**: Aggregated search across providers
- **Direct Booking**: Airline/hotel direct channels
- **Package Deals**: Bundled travel components
- **Loyalty Programs**: Points-based rewards
- **Dynamic Pricing**: Demand-based fare optimization

## Advanced Features

- **Price Prediction**: ML-based fare forecasting
- **Flexible Dates**: Calendar view of prices
- **Multi-City Search**: Complex routing
- **Group Bookings**: Discounts for multiple travelers
- **Corporate Travel**: Business travel management
- **Travel Insurance**: Optional coverage
- **Seat Selection**: Interactive seat maps
- **Special Requests**: Meals, assistance, preferences

## Integration Patterns

### Cross-Protocol Data Flow
1. **Search**: Redis cache → PostgreSQL query → Redis cache update
2. **Booking**: PostgreSQL write → Redis inventory update → MongoDB preferences
3. **Loyalty**: Redis points → PostgreSQL transaction → Neo4j graph
4. **Analytics**: PostgreSQL → Cassandra aggregation → Redis metrics

### Real-Time Updates
- **Inventory**: PostgreSQL triggers → Redis cache invalidation
- **Pricing**: Redis pub/sub → Client notifications
- **Availability**: Redis counters → PostgreSQL sync

## License

Part of the Orbit-RS project (MIT OR BSD-3-Clause)

---

**Built with Orbit-RS - One Server, All Protocols** 🚀
