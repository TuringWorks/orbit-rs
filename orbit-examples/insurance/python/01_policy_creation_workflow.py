#!/usr/bin/env python3
"""
OrbitRS Insurance Examples - End-to-End Auto Policy Creation Workflow
=======================================================================
Demonstrates multi-protocol integration across PostgreSQL, Redis, MongoDB, and Neo4j
for creating an auto insurance policy with all supporting data.
"""

import os
import sys
import psycopg2
import redis
import pymongo
from neo4j import GraphDatabase
import json
import uuid
from datetime import datetime, timedelta
from decimal import Decimal

# Import shared configuration helpers from the common example utilities.
sys.path.insert(
    0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
)
from orbit_utils import require_env, env_int


class InsurancePolicyWorkflow:
    """End-to-end auto insurance policy creation using multiple OrbitRS protocols"""

    def __init__(self):
        # Connect to OrbitRS PostgreSQL. Credentials come from environment
        # variables; never hardcode database passwords in source code.
        self.pg_conn = psycopg2.connect(
            host=os.getenv("ORBIT_PG_HOST", "localhost"),
            port=env_int("ORBIT_PG_PORT", 5432),
            database=os.getenv("ORBIT_PG_DB", "insurance"),
            user=os.getenv("ORBIT_PG_USER", "orbit"),
            password=require_env("ORBIT_PG_PASSWORD"),
        )

        # Connect to OrbitRS Redis
        self.redis_client = redis.Redis(
            host=os.getenv("ORBIT_REDIS_HOST", "localhost"),
            port=env_int("ORBIT_REDIS_PORT", 6379),
            db=0,
            decode_responses=True
        )

        # Connect to OrbitRS MongoDB
        self.mongo_client = pymongo.MongoClient(os.getenv("ORBIT_MONGO_URL", "mongodb://localhost:27017/"))
        self.mongo_db = self.mongo_client["insurance"]

        # Connect to OrbitRS Neo4j. Credentials come from environment variables.
        self.neo4j_driver = GraphDatabase.driver(
            os.getenv("ORBIT_NEO4J_URL", "bolt://localhost:7687"),
            auth=(os.getenv("ORBIT_NEO4J_USER", "neo4j"), require_env("ORBIT_NEO4J_PASSWORD")),
        )

    def create_auto_policy(self, customer_data, vehicle_data, driver_data, coverage_data):
        """
        Create complete auto insurance policy across all protocols

        Workflow:
        1. PostgreSQL: Create customer, vehicle, driver, policy records
        2. Redis: Cache quote and risk scores
        3. MongoDB: Store policy documents
        4. Neo4j: Create relationship graph
        """

        print("=" * 80)
        print("AUTO INSURANCE POLICY CREATION WORKFLOW")
        print("=" * 80)

        # Step 1: Create customer in PostgreSQL
        print("\n[1/8] Creating customer in PostgreSQL...")
        customer_id = self._create_customer(customer_data)
        print(f"✓ Customer created: {customer_id}")

        # Step 2: Create vehicle in PostgreSQL
        print("\n[2/8] Creating vehicle in PostgreSQL...")
        vehicle_id = self._create_vehicle(customer_id, vehicle_data)
        print(f"✓ Vehicle created: {vehicle_id}")

        # Step 3: Create driver in PostgreSQL
        print("\n[3/8] Creating driver in PostgreSQL...")
        driver_id = self._create_driver(customer_id, driver_data)
        print(f"✓ Driver created: {driver_id}")

        # Step 4: Calculate and cache quote in Redis
        print("\n[4/8] Calculating quote and caching in Redis...")
        quote = self._calculate_and_cache_quote(
            customer_id, vehicle_id, driver_id, coverage_data
        )
        print(f"✓ Quote calculated: ${quote['premium']['annual']:.2f}/year")
        print(f"  Monthly: ${quote['premium']['monthly']:.2f}")

        # Step 5: Create policy in PostgreSQL
        print("\n[5/8] Creating policy in PostgreSQL...")
        policy_id = self._create_policy(customer_id, quote)
        print(f"✓ Policy created: {policy_id}")

        # Step 6: Link vehicle and driver to policy
        print("\n[6/8] Linking vehicle and driver to policy...")
        self._link_policy_vehicle_driver(policy_id, vehicle_id, driver_id)
        print("✓ Vehicle and driver linked to policy")

        # Step 7: Store policy documents in MongoDB
        print("\n[7/8] Storing policy documents in MongoDB...")
        doc_ids = self._store_policy_documents(policy_id, customer_data, vehicle_data)
        print(f"✓ {len(doc_ids)} documents stored in MongoDB")

        # Step 8: Create relationship graph in Neo4j
        print("\n[8/8] Creating relationship graph in Neo4j...")
        self._create_neo4j_relationships(customer_id, policy_id, vehicle_id)
        print("✓ Relationship graph created in Neo4j")

        print("\n" + "=" * 80)
        print("POLICY CREATION COMPLETE!")
        print("=" * 80)
        print(f"\nPolicy ID: {policy_id}")
        print(f"Customer: {customer_data['first_name']} {customer_data['last_name']}")
        print(f"Vehicle: {vehicle_data['year']} {vehicle_data['make']} {vehicle_data['model']}")
        print(f"Annual Premium: ${quote['premium']['annual']:.2f}")
        print(f"Coverage: {coverage_data['liability_bi_per_person']}/{coverage_data['liability_bi_per_accident']}/{coverage_data['liability_pd']}")

        return {
            'policy_id': policy_id,
            'customer_id': customer_id,
            'vehicle_id': vehicle_id,
            'driver_id': driver_id,
            'quote': quote,
            'document_ids': doc_ids
        }

    def _create_customer(self, customer_data):
        """Create customer in PostgreSQL"""
        customer_id = str(uuid.uuid4())
        customer_number = f"CUST-{datetime.now().strftime('%Y%m%d')}-{customer_id[:8]}"

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO customers (
                    customer_id, customer_number, first_name, last_name,
                    date_of_birth, email, phone, credit_score
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                customer_id, customer_number,
                customer_data['first_name'], customer_data['last_name'],
                customer_data['date_of_birth'], customer_data['email'],
                customer_data['phone'], customer_data.get('credit_score', 700)
            ))
            self.pg_conn.commit()

        return customer_id

    def _create_vehicle(self, customer_id, vehicle_data):
        """Create vehicle in PostgreSQL"""
        vehicle_id = str(uuid.uuid4())

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO vehicles (
                    vehicle_id, vin, customer_id, year, make, model,
                    body_style, fuel_type, annual_mileage, primary_use
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                vehicle_id, vehicle_data['vin'], customer_id,
                vehicle_data['year'], vehicle_data['make'], vehicle_data['model'],
                vehicle_data.get('body_style', 'SEDAN'),
                vehicle_data.get('fuel_type', 'GASOLINE'),
                vehicle_data.get('annual_mileage', 12000),
                vehicle_data.get('primary_use', 'PERSONAL')
            ))
            self.pg_conn.commit()

        return vehicle_id

    def _create_driver(self, customer_id, driver_data):
        """Create driver in PostgreSQL"""
        driver_id = str(uuid.uuid4())

        with self.pg_conn.cursor() as cursor:
            cursor.execute("""
                INSERT INTO drivers (
                    driver_id, customer_id, first_name, last_name,
                    date_of_birth, license_number, license_state,
                    years_licensed, relationship_to_policyholder
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                driver_id, customer_id,
                driver_data['first_name'], driver_data['last_name'],
                driver_data['date_of_birth'], driver_data['license_number'],
                driver_data['license_state'], driver_data.get('years_licensed', 10),
                'SELF'
            ))
            self.pg_conn.commit()

        return driver_id

    def _calculate_and_cache_quote(self, customer_id, vehicle_id, driver_id, coverage_data):
        """Calculate premium and cache quote in Redis"""

        # Simple premium calculation (in production, this would be much more complex)
        base_premium = 1200.00

        # Apply factors
        coverage_factor = coverage_data['liability_bi_per_person'] / 100000
        collision_factor = 1.3 if coverage_data.get('collision_coverage') else 1.0
        comprehensive_factor = 1.15 if coverage_data.get('comprehensive_coverage') else 1.0

        annual_premium = base_premium * coverage_factor * collision_factor * comprehensive_factor

        # Apply discounts
        discounts = []
        discount_amount = 0

        if coverage_data.get('multi_policy_discount'):
            discounts.append('multi_policy')
            discount_amount += annual_premium * 0.10

        if coverage_data.get('good_driver_discount'):
            discounts.append('good_driver')
            discount_amount += annual_premium * 0.05

        final_premium = annual_premium - discount_amount

        quote = {
            'quote_id': f"Q-{datetime.now().strftime('%Y-%m%d%H%M%S')}",
            'customer_id': customer_id,
            'vehicle_id': vehicle_id,
            'driver_id': driver_id,
            'coverage': coverage_data,
            'premium': {
                'annual': round(final_premium, 2),
                'semi_annual': round(final_premium / 2, 2),
                'monthly': round(final_premium / 12, 2)
            },
            'discounts': discounts,
            'quote_expires_at': (datetime.now() + timedelta(days=30)).isoformat(),
            'generated_at': datetime.now().isoformat()
        }

        # Cache in Redis (TTL: 30 minutes)
        cache_key = f"quote:auto:{customer_id}:{vehicle_id}"
        self.redis_client.setex(
            cache_key,
            1800,  # 30 minutes
            json.dumps(quote, default=str)
        )

        # Also cache risk score
        risk_score = {
            'customer_id': customer_id,
            'overall_risk_score': 72,
            'auto_risk_score': 68,
            'calculated_at': datetime.now().isoformat()
        }
        self.redis_client.setex(
            f"risk:customer:{customer_id}",
            3600,  # 1 hour
            json.dumps(risk_score)
        )

        return quote

    def _create_policy(self, customer_id, quote):
        """Create policy in PostgreSQL"""
        policy_id = str(uuid.uuid4())
        policy_number = f"AUTO-{datetime.now().strftime('%Y-%m%d')}-{policy_id[:8]}"

        effective_date = datetime.now().date()
        expiration_date = effective_date + timedelta(days=365)

        with self.pg_conn.cursor() as cursor:
            # Create main policy
            cursor.execute("""
                INSERT INTO policies (
                    policy_id, policy_number, customer_id, policy_type,
                    policy_status, effective_date, expiration_date,
                    premium_amount, coverage_amount, risk_score
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                policy_id, policy_number, customer_id, 'AUTO',
                'ACTIVE', effective_date, expiration_date,
                quote['premium']['annual'],
                quote['coverage']['liability_bi_per_accident'],
                72  # Risk score
            ))

            # Create auto policy details
            cursor.execute("""
                INSERT INTO auto_policies (
                    auto_policy_id, policy_id,
                    liability_bodily_injury_per_person,
                    liability_bodily_injury_per_accident,
                    liability_property_damage,
                    collision_coverage, collision_deductible,
                    comprehensive_coverage, comprehensive_deductible,
                    multi_policy_discount, good_driver_discount
                ) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s)
            """, (
                str(uuid.uuid4()), policy_id,
                quote['coverage']['liability_bi_per_person'],
                quote['coverage']['liability_bi_per_accident'],
                quote['coverage']['liability_pd'],
                quote['coverage'].get('collision_coverage', False),
                quote['coverage'].get('collision_deductible', 500),
                quote['coverage'].get('comprehensive_coverage', False),
                quote['coverage'].get('comprehensive_deductible', 500),
                'multi_policy' in quote['discounts'],
                'good_driver' in quote['discounts']
            ))

            self.pg_conn.commit()

        return policy_id

    def _link_policy_vehicle_driver(self, policy_id, vehicle_id, driver_id):
        """Link vehicle and driver to policy"""
        with self.pg_conn.cursor() as cursor:
            # Link vehicle
            cursor.execute("""
                INSERT INTO policy_vehicles (
                    policy_vehicle_id, policy_id, vehicle_id,
                    coverage_type, primary_driver_id
                ) VALUES (%s, %s, %s, %s, %s)
            """, (
                str(uuid.uuid4()), policy_id, vehicle_id,
                'FULL', driver_id
            ))

            # Link driver
            cursor.execute("""
                INSERT INTO policy_drivers (
                    policy_driver_id, policy_id, driver_id, driver_status
                ) VALUES (%s, %s, %s, %s)
            """, (
                str(uuid.uuid4()), policy_id, driver_id, 'LISTED'
            ))

            self.pg_conn.commit()

    def _store_policy_documents(self, policy_id, customer_data, vehicle_data):
        """Store policy documents in MongoDB"""
        doc_ids = []

        # Policy contract document
        contract_doc = {
            'policy_id': policy_id,
            'document_type': 'policy_contract',
            'file_name': f'{policy_id}-Contract.pdf',
            'file_size_bytes': 524288,
            'mime_type': 'application/pdf',
            'storage_url': f's3://insurance-docs/policies/{policy_id}-Contract.pdf',
            'metadata': {
                'customer_name': f"{customer_data['first_name']} {customer_data['last_name']}",
                'vehicle': f"{vehicle_data['year']} {vehicle_data['make']} {vehicle_data['model']}"
            },
            'uploaded_by': 'system',
            'uploaded_at': datetime.now()
        }
        result = self.mongo_db.policy_documents.insert_one(contract_doc)
        doc_ids.append(str(result.inserted_id))

        # Declaration page
        declaration_doc = {
            'policy_id': policy_id,
            'document_type': 'declaration',
            'file_name': f'{policy_id}-Declarations.pdf',
            'file_size_bytes': 102400,
            'mime_type': 'application/pdf',
            'storage_url': f's3://insurance-docs/policies/{policy_id}-Declarations.pdf',
            'uploaded_by': 'system',
            'uploaded_at': datetime.now()
        }
        result = self.mongo_db.policy_documents.insert_one(declaration_doc)
        doc_ids.append(str(result.inserted_id))

        return doc_ids

    def _create_neo4j_relationships(self, customer_id, policy_id, vehicle_id):
        """Create relationship graph in Neo4j"""
        with self.neo4j_driver.session() as session:
            # Create customer node
            session.run("""
                MERGE (c:Customer {customer_id: $customer_id})
                SET c.created_at = datetime()
            """, customer_id=customer_id)

            # Create policy node
            session.run("""
                MERGE (p:Policy {policy_id: $policy_id})
                SET p.policy_type = 'AUTO',
                    p.status = 'ACTIVE',
                    p.created_at = datetime()
            """, policy_id=policy_id)

            # Create vehicle node
            session.run("""
                MERGE (v:Vehicle {vehicle_id: $vehicle_id})
                SET v.created_at = datetime()
            """, vehicle_id=vehicle_id)

            # Create relationships
            session.run("""
                MATCH (c:Customer {customer_id: $customer_id})
                MATCH (p:Policy {policy_id: $policy_id})
                MERGE (c)-[:HAS_POLICY {since: date()}]->(p)
            """, customer_id=customer_id, policy_id=policy_id)

            session.run("""
                MATCH (p:Policy {policy_id: $policy_id})
                MATCH (v:Vehicle {vehicle_id: $vehicle_id})
                MERGE (p)-[:COVERS {since: date()}]->(v)
            """, policy_id=policy_id, vehicle_id=vehicle_id)

    def close_connections(self):
        """Close all database connections"""
        self.pg_conn.close()
        self.redis_client.close()
        self.mongo_client.close()
        self.neo4j_driver.close()


def main():
    """Example usage"""

    # Sample customer data
    customer_data = {
        'first_name': 'John',
        'last_name': 'Doe',
        'date_of_birth': '1985-06-15',
        'email': 'john.doe@email.com',
        'phone': '+14155550123',
        'credit_score': 720
    }

    # Sample vehicle data
    vehicle_data = {
        'vin': '1HGCM82633A123456',
        'year': 2023,
        'make': 'Honda',
        'model': 'Accord',
        'body_style': 'SEDAN',
        'fuel_type': 'GASOLINE',
        'annual_mileage': 12000,
        'primary_use': 'PERSONAL'
    }

    # Sample driver data
    driver_data = {
        'first_name': 'John',
        'last_name': 'Doe',
        'date_of_birth': '1985-06-15',
        'license_number': 'D1234567',
        'license_state': 'CA',
        'years_licensed': 17
    }

    # Sample coverage data
    coverage_data = {
        'liability_bi_per_person': 250000,
        'liability_bi_per_accident': 500000,
        'liability_pd': 100000,
        'collision_coverage': True,
        'collision_deductible': 500,
        'comprehensive_coverage': True,
        'comprehensive_deductible': 500,
        'multi_policy_discount': True,
        'good_driver_discount': True
    }

    # Create workflow instance
    workflow = InsurancePolicyWorkflow()

    try:
        # Execute policy creation workflow
        result = workflow.create_auto_policy(
            customer_data, vehicle_data, driver_data, coverage_data
        )

        print("\n" + "=" * 80)
        print("WORKFLOW RESULT:")
        print(json.dumps(result, indent=2, default=str))

    finally:
        workflow.close_connections()


if __name__ == '__main__':
    main()
