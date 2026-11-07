ARG PHP_VERSION
FROM php:${PHP_VERSION}-fpm-trixie

ARG XDEBUG_VERSION
RUN pecl install "xdebug-${XDEBUG_VERSION}" && docker-php-ext-enable xdebug \
    && mkdir -p /var/log/xdebug && chown www-data:www-data /var/log/xdebug

COPY xdebug.ini /usr/local/etc/php/conf.d/zz-xdebug.ini
COPY app.ini /usr/local/etc/php/conf.d/zz-app.ini
