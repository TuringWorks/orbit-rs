Feature: Distributed Actor Messaging
  As a developer building distributed systems
  I want actors to communicate seamlessly across nodes
  So that I can build scalable applications

  Background:
    Given an Orbit cluster with 3 nodes is running
    And multiple clients are connected to different nodes

  Scenario: Send message to actor on same node
    Given an actor "MessageActor" with key "msg-1" is on node 1
    And a client is connected to node 1
    When the client sends a message "Hello Local" to the actor
    Then the message should be delivered directly
    And the response time should be minimal

  Scenario: Send message to actor on different node
    Given an actor "MessageActor" with key "msg-2" is on node 2
    And a client is connected to node 1
    When the client sends a message "Hello Remote" to the actor
    Then the message should be routed to node 2
    And the actor should process the message successfully

  Scenario: Actor migration between nodes
    Given an actor "MobileActor" with key "mobile-1" is on node 1
    When node 1 becomes overloaded
    Then the actor should migrate to a less loaded node
    And messages should continue to be delivered correctly

  Scenario: Message ordering guarantees
    Given an actor "SequenceActor" with key "seq-1" exists
    When multiple clients send ordered messages concurrently
    Then the actor should process messages in the correct order
    And no messages should be lost

  Scenario: Broadcast messaging
    Given multiple actors of type "NotificationActor" exist
    When a coordinator sends a broadcast message
    Then all actors should receive the message
    And each actor should process it independently