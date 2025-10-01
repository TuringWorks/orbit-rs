Feature: Actor Lifecycle Management
  As a developer using Orbit
  I want actors to be created, activated, and deactivated automatically
  So that I can focus on business logic rather than lifecycle management

  Background:
    Given an Orbit server is running
    And an Orbit client is connected

  Scenario: Create and activate an actor
    Given no actor with key "user-123" exists
    When I send a message to actor "UserActor" with key "user-123"
    Then the actor should be created and activated
    And the message should be processed successfully

  Scenario: Actor state persistence
    Given an actor "CounterActor" with key "counter-1" exists
    When I increment the counter by 5
    And I increment the counter by 3
    Then the counter value should be 8
    And the actor should have processed 2 messages

  Scenario: Actor deactivation after timeout
    Given an actor "TimerActor" with key "timer-1" exists
    When the actor has been idle for more than the timeout period
    Then the actor should be automatically deactivated
    And its state should be persisted

  Scenario: Multiple actors with same type
    Given no actors exist
    When I create actors "UserActor" with keys "user-1", "user-2", and "user-3"
    Then each actor should maintain separate state
    And operations on one actor should not affect others

  Scenario: Actor reactivation
    Given an actor "DataActor" with key "data-1" was previously deactivated
    And the actor had some persistent state
    When I send a message to the actor
    Then the actor should be reactivated
    And its previous state should be restored