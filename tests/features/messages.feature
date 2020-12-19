Feature: Send and get messages

  Scenario: A user wants to send a message in a channel
    Given wirc is running on localhost
    Given the user is in a guild and has access to a text channel
    When a user sends a message in a channel in a guild
    Then the user should their message in the channel

  Scenario: A user wants to get the last messages sent in a channel
    Given wirc is running on localhost
    Given the user is in a guild and has access to a text channel
    Given the user has sent 3 messages in the channel
    When a user tries to get the last 2 messages
    Then the user should see 2 messages