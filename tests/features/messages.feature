Feature: Send and get messages

    Scenario: An authenticated user wants to send a message in a channel
        Given the user has a text channel
        When the user sends a message in a channel in a hub
        Then the user should receive an ID

    Scenario: An authenticated user wants to get the last messages sent in a channel
        Given the user has sent 3 messages in a text channel
        When the user tries to get the last 2 messages
        Then the user should see 2 messages
