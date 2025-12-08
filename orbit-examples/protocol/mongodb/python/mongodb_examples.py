"""
============================================================================
Orbit-RS MongoDB Protocol Examples - Python Client
============================================================================
This file demonstrates MongoDB operations using PyMongo with Orbit-RS.

Prerequisites:
1. Start Orbit server: cargo run --bin orbit-server
2. Install PyMongo: pip install pymongo
3. Run: python mongodb_examples.py
============================================================================
"""

from pymongo import MongoClient, ASCENDING, DESCENDING
from datetime import datetime
from pprint import pprint

print("=" * 80)
print("MongoDB Python Client Examples with Orbit-RS")
print("=" * 80)

# ============================================================================
# 1. CONNECTION
# ============================================================================

print("\n1. CONNECTION")
print("-" * 80)

# Connect to Orbit-RS MongoDB protocol
client = MongoClient('mongodb://localhost:27017/')
db = client['orbit_examples']

print("Connected to Orbit-RS via MongoDB protocol")
print(f"Database: {db.name}")

# ============================================================================
# 2. BASIC CRUD OPERATIONS
# ============================================================================

print("\n\n2. BASIC CRUD OPERATIONS")
print("-" * 80)

# Drop collection for clean start
db.employees.drop()

# Insert one document
print("\n2.1 Insert One Document:")
employee = {
    "employee_id": "E001",
    "name": "Alice Johnson",
    "department": "Engineering",
    "position": "Senior Developer",
    "salary": 120000,
    "skills": ["Python", "Rust", "MongoDB"],
    "hire_date": datetime(2020, 1, 15),
    "active": True
}

result = db.employees.insert_one(employee)
print(f"Inserted document ID: {result.inserted_id}")

# Insert many documents
print("\n2.2 Insert Many Documents:")
employees = [
    {
        "employee_id": "E002",
        "name": "Bob Smith",
        "department": "Engineering",
        "position": "Developer",
        "salary": 95000,
        "skills": ["JavaScript", "React", "Node.js"],
        "hire_date": datetime(2021, 3, 20),
        "active": True
    },
    {
        "employee_id": "E003",
        "name": "Carol White",
        "department": "Product",
        "position": "Product Manager",
        "salary": 110000,
        "skills": ["Product Management", "Agile", "Analytics"],
        "hire_date": datetime(2019, 6, 10),
        "active": True
    },
    {
        "employee_id": "E004",
        "name": "David Brown",
        "department": "Engineering",
        "position": "DevOps Engineer",
        "salary": 105000,
        "skills": ["Kubernetes", "Docker", "AWS"],
        "hire_date": datetime(2020, 9, 5),
        "active": True
    },
    {
        "employee_id": "E005",
        "name": "Eve Davis",
        "department": "Sales",
        "position": "Sales Manager",
        "salary": 115000,
        "skills": ["Sales", "CRM", "Negotiation"],
        "hire_date": datetime(2018, 2, 1),
        "active": False
    }
]

result = db.employees.insert_many(employees)
print(f"Inserted {len(result.inserted_ids)} documents")

# Find all documents
print("\n2.3 Find All Employees:")
for emp in db.employees.find():
    print(f"  - {emp['name']} ({emp['department']}): ${emp['salary']:,}")

# Find with filter
print("\n2.4 Find Engineering Department:")
for emp in db.employees.find({"department": "Engineering"}):
    print(f"  - {emp['name']}: {emp['position']}")

# Find one document
print("\n2.5 Find One Employee:")
emp = db.employees.find_one({"employee_id": "E001"})
print(f"  Found: {emp['name']}, {emp['position']}")

# Count documents
print("\n2.6 Count Documents:")
total = db.employees.count_documents({})
engineering = db.employees.count_documents({"department": "Engineering"})
print(f"  Total employees: {total}")
print(f"  Engineering: {engineering}")

# Update one document
print("\n2.7 Update One Document (Salary Increase):")
result = db.employees.update_one(
    {"employee_id": "E002"},
    {"$set": {"salary": 100000}, "$currentDate": {"updated_at": True}}
)
print(f"  Matched: {result.matched_count}, Modified: {result.modified_count}")

# Update many documents
print("\n2.8 Update Many Documents (Department Bonus):")
result = db.employees.update_many(
    {"department": "Engineering"},
    {"$inc": {"salary": 5000}}
)
print(f"  Matched: {result.matched_count}, Modified: {result.modified_count}")

# Delete one document
print("\n2.9 Delete Inactive Employees:")
result = db.employees.delete_many({"active": False})
print(f"  Deleted: {result.deleted_count} document(s)")

# ============================================================================
# 3. ADVANCED QUERIES
# ============================================================================

print("\n\n3. ADVANCED QUERIES")
print("-" * 80)

# Comparison operators
print("\n3.1 Employees with Salary > $100,000:")
for emp in db.employees.find({"salary": {"$gt": 100000}}):
    print(f"  - {emp['name']}: ${emp['salary']:,}")

# Logical operators
print("\n3.2 Engineering with Salary > $100,000:")
for emp in db.employees.find({
    "$and": [
        {"department": "Engineering"},
        {"salary": {"$gt": 100000}}
    ]
}):
    print(f"  - {emp['name']}: ${emp['salary']:,}")

# Array operators
print("\n3.3 Employees with Python Skills:")
for emp in db.employees.find({"skills": "Python"}):
    print(f"  - {emp['name']}: {', '.join(emp['skills'])}")

# Projection (select specific fields)
print("\n3.4 Employee Names and Positions:")
for emp in db.employees.find({}, {"name": 1, "position": 1, "_id": 0}):
    print(f"  - {emp['name']}: {emp['position']}")

# Sorting
print("\n3.5 Employees Sorted by Salary (Descending):")
for emp in db.employees.find().sort("salary", DESCENDING):
    print(f"  - {emp['name']}: ${emp['salary']:,}")

# Limit and skip
print("\n3.6 Top 3 Highest Paid Employees:")
for emp in db.employees.find().sort("salary", DESCENDING).limit(3):
    print(f"  - {emp['name']}: ${emp['salary']:,}")

# ============================================================================
# 4. AGGREGATION PIPELINE
# ============================================================================

print("\n\n4. AGGREGATION PIPELINE")
print("-" * 80)

# Group by department
print("\n4.1 Average Salary by Department:")
pipeline = [
    {
        "$group": {
            "_id": "$department",
            "avg_salary": {"$avg": "$salary"},
            "count": {"$sum": 1}
        }
    },
    {"$sort": {"avg_salary": -1}}
]

for doc in db.employees.aggregate(pipeline):
    print(f"  {doc['_id']}: ${doc['avg_salary']:,.2f} (n={doc['count']})")

# Complex aggregation
print("\n4.2 Department Statistics:")
pipeline = [
    {
        "$group": {
            "_id": "$department",
            "avg_salary": {"$avg": "$salary"},
            "min_salary": {"$min": "$salary"},
            "max_salary": {"$max": "$salary"},
            "total_payroll": {"$sum": "$salary"},
            "count": {"$sum": 1}
        }
    },
    {"$sort": {"total_payroll": -1}}
]

for doc in db.employees.aggregate(pipeline):
    print(f"\n  {doc['_id']}:")
    print(f"    Count: {doc['count']}")
    print(f"    Avg Salary: ${doc['avg_salary']:,.2f}")
    print(f"    Min Salary: ${doc['min_salary']:,}")
    print(f"    Max Salary: ${doc['max_salary']:,}")
    print(f"    Total Payroll: ${doc['total_payroll']:,}")

# ============================================================================
# 5. INDEXES
# ============================================================================

print("\n\n5. INDEXES")
print("-" * 80)

# Create single field index
print("\n5.1 Create Index on employee_id:")
db.employees.create_index([("employee_id", ASCENDING)], unique=True)
print("  Index created")

# Create compound index
print("\n5.2 Create Compound Index on department and salary:")
db.employees.create_index([("department", ASCENDING), ("salary", DESCENDING)])
print("  Compound index created")

# List indexes
print("\n5.3 List All Indexes:")
for index in db.employees.list_indexes():
    print(f"  - {index['name']}: {index['key']}")

# ============================================================================
# 6. BULK OPERATIONS
# ============================================================================

print("\n\n6. BULK OPERATIONS")
print("-" * 80)

from pymongo import InsertOne, UpdateOne, DeleteOne

print("\n6.1 Bulk Write Operations:")
bulk_ops = [
    InsertOne({
        "employee_id": "E006",
        "name": "Frank Miller",
        "department": "Marketing",
        "position": "Marketing Manager",
        "salary": 100000,
        "skills": ["Marketing", "SEO", "Content"],
        "hire_date": datetime(2022, 1, 10),
        "active": True
    }),
    UpdateOne(
        {"employee_id": "E001"},
        {"$set": {"position": "Lead Developer"}}
    ),
    UpdateOne(
        {"employee_id": "E002"},
        {"$inc": {"salary": 5000}}
    )
]

result = db.employees.bulk_write(bulk_ops)
print(f"  Inserted: {result.inserted_count}")
print(f"  Modified: {result.modified_count}")

# ============================================================================
# 7. TRANSACTIONS (if supported)
# ============================================================================

print("\n\n7. TRANSACTIONS")
print("-" * 80)

print("\n7.1 Multi-Document Transaction:")
try:
    with client.start_session() as session:
        with session.start_transaction():
            # Transfer operation example
            db.employees.update_one(
                {"employee_id": "E001"},
                {"$inc": {"salary": -10000}},
                session=session
            )
            db.employees.update_one(
                {"employee_id": "E002"},
                {"$inc": {"salary": 10000}},
                session=session
            )
            print("  Transaction committed successfully")
except Exception as e:
    print(f"  Transaction failed: {e}")

# ============================================================================
# 8. TEXT SEARCH
# ============================================================================

print("\n\n8. TEXT SEARCH")
print("-" * 80)

# Create text index
print("\n8.1 Create Text Index on skills:")
try:
    db.employees.create_index([("skills", "text")])
    print("  Text index created")
    
    # Text search
    print("\n8.2 Search for 'Python' in skills:")
    for emp in db.employees.find({"$text": {"$search": "Python"}}):
        print(f"  - {emp['name']}: {', '.join(emp['skills'])}")
except Exception as e:
    print(f"  Text search not supported: {e}")

# ============================================================================
# 9. SUMMARY
# ============================================================================

print("\n\n9. SUMMARY")
print("-" * 80)

total_employees = db.employees.count_documents({})
active_employees = db.employees.count_documents({"active": True})

pipeline = [
    {
        "$group": {
            "_id": None,
            "avg_salary": {"$avg": "$salary"},
            "total_payroll": {"$sum": "$salary"}
        }
    }
]

stats = list(db.employees.aggregate(pipeline))[0]

print(f"Total Employees: {total_employees}")
print(f"Active Employees: {active_employees}")
print(f"Average Salary: ${stats['avg_salary']:,.2f}")
print(f"Total Payroll: ${stats['total_payroll']:,}")

print("\nDepartment Breakdown:")
pipeline = [
    {"$group": {"_id": "$department", "count": {"$sum": 1}}},
    {"$sort": {"count": -1}}
]

for doc in db.employees.aggregate(pipeline):
    print(f"  - {doc['_id']}: {doc['count']}")

# Close connection
client.close()

print("\n" + "=" * 80)
print("MongoDB Python Client Examples Complete!")
print("=" * 80)
