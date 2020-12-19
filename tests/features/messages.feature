Feature: Message sending and getting
  Scenario: A user wants to send a message in a channel
    Given wirc is running on localhost
    Given a guild has been created with 1 channel
    When a user sends a message in a channel in a guild
    Then the user should their message in the channel

  Scenario: A user wants to get the last messages sent in a channel
    Given wirc is running on localhost
    Given a guild has been created with 1 channel
    Given 3 messages have been sent in a channel
    When a user tries to get the last 2 messages
    Then the user should see 2 messages